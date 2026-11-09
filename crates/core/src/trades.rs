// SPDX-License-Identifier: Apache-2.0
//! Trades derived from ledger meta, following Horizon's
//! `internal/ingest/processors/trades_processor.go`:
//!
//! * only successful transactions; ClaimAtoms of PathPaymentStrictReceive,
//!   PathPaymentStrictSend, ManageBuyOffer, ManageSellOffer and
//!   CreatePassiveSellOffer results (the op type is read from the result arm,
//!   which also covers core's wrong-arm passive-offer results);
//! * zeroed atoms (offers garbage-collected by core) are skipped;
//! * base = asset sold by the maker, counter = asset bought;
//! * price of an order-book trade = the maker offer's price in the operation's
//!   STATE pre-image; price of a pool trade = amount_bought / amount_sold;
//! * pool trades carry Horizon's rounding slippage (bips), computed from the
//!   pool pre-image; `/trade_aggregations` drops trades with slippage >= 1000.

use crate::asset;
use crate::ledger::{tx_changes, LedgerView};
use crate::pool_math::{calculate_pool_expectation, calculate_pool_payout};
use serde::{Deserialize, Serialize};
use stellar_xdr::{
    ClaimAtom, InnerTransactionResultResult, LedgerEntry, LedgerEntryChange, LedgerEntryChanges, LedgerEntryData, LedgerKey,
    LiquidityPoolEntryBody, ManageBuyOfferResult, ManageSellOfferResult, OperationResult, OperationResultTr,
    PathPaymentStrictReceiveResult, PathPaymentStrictSendResult, TransactionResult, TransactionResultResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    pub ledger: u32,
    /// Ledger close time, unix seconds.
    pub close_time: u64,
    /// Horizon TOID of the operation: ledger << 32 | tx_index << 12 | (op_index + 1).
    pub op_id: i64,
    /// Position of the atom in the operation's result.
    pub order: u32,
    pub op_type: String,
    /// Asset sold by the maker (offer owner or pool) = Horizon base before reordering.
    pub sold: String,
    pub bought: String,
    pub amount_sold: i64,
    pub amount_bought: i64,
    pub price_n: i64,
    pub price_d: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seller: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounding_slippage: Option<i64>,
}

pub fn toid(ledger: u32, tx_index: u32, op_index: u32) -> i64 {
    ((ledger as i64) << 32) | ((tx_index as i64) << 12) | (op_index as i64)
}

fn op_results(r: &TransactionResult) -> Option<&[OperationResult]> {
    match &r.result {
        TransactionResultResult::TxSuccess(ops) => Some(ops.as_slice()),
        TransactionResultResult::TxFeeBumpInnerSuccess(inner) => match &inner.result.result {
            InnerTransactionResultResult::TxSuccess(ops) => Some(ops.as_slice()),
            _ => None,
        },
        _ => None,
    }
}

/// (op type name, claim atoms) for trade-producing results.
fn atoms(r: &OperationResult) -> Option<(&'static str, &[ClaimAtom])> {
    let OperationResult::OpInner(tr) = r else { return None };
    match tr {
        OperationResultTr::PathPaymentStrictReceive(PathPaymentStrictReceiveResult::Success(s)) => {
            Some(("path_payment_strict_receive", s.offers.as_slice()))
        }
        OperationResultTr::PathPaymentStrictSend(PathPaymentStrictSendResult::Success(s)) => {
            Some(("path_payment_strict_send", s.offers.as_slice()))
        }
        OperationResultTr::ManageBuyOffer(ManageBuyOfferResult::Success(s)) => Some(("manage_buy_offer", s.offers_claimed.as_slice())),
        OperationResultTr::ManageSellOffer(ManageSellOfferResult::Success(s)) => Some(("manage_sell_offer", s.offers_claimed.as_slice())),
        OperationResultTr::CreatePassiveSellOffer(ManageSellOfferResult::Success(s)) => {
            Some(("create_passive_sell_offer", s.offers_claimed.as_slice()))
        }
        _ => None,
    }
}

/// Last STATE pre-image in `changes` whose key equals `key` (Horizon `findOperationChange`).
fn find_pre<'a>(changes: &'a LedgerEntryChanges, key: &LedgerKey) -> Option<&'a LedgerEntry> {
    changes.iter().rev().find_map(|c| match c {
        LedgerEntryChange::State(e) if &e.to_key() == key => Some(e),
        _ => None,
    })
}

struct AtomView {
    sold: stellar_xdr::Asset,
    bought: stellar_xdr::Asset,
    amount_sold: i64,
    amount_bought: i64,
    offer: Option<(stellar_xdr::AccountId, i64)>,
    pool: Option<stellar_xdr::PoolId>,
}

fn atom_view(a: &ClaimAtom) -> AtomView {
    match a {
        ClaimAtom::V0(x) => AtomView {
            sold: x.asset_sold.clone(),
            bought: x.asset_bought.clone(),
            amount_sold: x.amount_sold,
            amount_bought: x.amount_bought,
            offer: Some((stellar_xdr::AccountId(stellar_xdr::PublicKey::PublicKeyTypeEd25519(x.seller_ed25519.clone())), x.offer_id)),
            pool: None,
        },
        ClaimAtom::OrderBook(x) => AtomView {
            sold: x.asset_sold.clone(),
            bought: x.asset_bought.clone(),
            amount_sold: x.amount_sold,
            amount_bought: x.amount_bought,
            offer: Some((x.seller_id.clone(), x.offer_id)),
            pool: None,
        },
        ClaimAtom::LiquidityPool(x) => AtomView {
            sold: x.asset_sold.clone(),
            bought: x.asset_bought.clone(),
            amount_sold: x.amount_sold,
            amount_bought: x.amount_bought,
            offer: None,
            pool: Some(x.liquidity_pool_id.clone()),
        },
    }
}

fn pool_slippage(op_type: &str, a: &AtomView, pre: &LedgerEntry) -> Option<i64> {
    let LedgerEntryData::LiquidityPool(lp) = &pre.data else { return None };
    let LiquidityPoolEntryBody::LiquidityPoolConstantProduct(cp) = &lp.body;
    let (mut disbursed_res, mut deposited_res) = (cp.reserve_a, cp.reserve_b);
    if a.sold != cp.params.asset_a {
        std::mem::swap(&mut disbursed_res, &mut deposited_res);
    }
    let r = match op_type {
        "path_payment_strict_receive" => calculate_pool_expectation(deposited_res, disbursed_res, a.amount_sold, cp.params.fee, true),
        "path_payment_strict_send" => calculate_pool_payout(deposited_res, disbursed_res, a.amount_bought, cp.params.fee, true),
        _ => return None,
    };
    // Horizon: on overflow/underflow set the maximum so the trade is excluded.
    Some(r.map_or(i64::MAX, |x| x.1))
}

/// All trades of one ledger plus messages for atoms whose pre-image was missing.
pub fn from_view(v: &LedgerView<'_>) -> (Vec<Trade>, Vec<String>) {
    let seq = v.sequence();
    let close = v.close_time();
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for tx in &v.txs {
        let Some(results) = op_results(tx.result) else { continue };
        let (_, op_changes, _) = tx_changes(tx.meta);
        for (opi, r) in results.iter().enumerate() {
            let Some((op_type, list)) = atoms(r) else { continue };
            let op_id = toid(seq, tx.index, opi as u32 + 1);
            let empty = LedgerEntryChanges::default();
            let changes = op_changes.get(opi).copied().unwrap_or(&empty);
            for (order, atom) in list.iter().enumerate() {
                let a = atom_view(atom);
                if a.amount_sold == 0 && a.amount_bought == 0 {
                    continue;
                }
                let mut t = Trade {
                    ledger: seq,
                    close_time: close,
                    op_id,
                    order: order as u32,
                    op_type: op_type.to_string(),
                    sold: asset::from_xdr(&a.sold),
                    bought: asset::from_xdr(&a.bought),
                    amount_sold: a.amount_sold,
                    amount_bought: a.amount_bought,
                    price_n: a.amount_bought,
                    price_d: a.amount_sold,
                    offer_id: None,
                    seller: None,
                    pool_id: None,
                    rounding_slippage: None,
                };
                if let Some((seller, id)) = &a.offer {
                    t.offer_id = Some(*id);
                    t.seller = Some(asset::account_str(seller));
                    let key = LedgerKey::Offer(stellar_xdr::LedgerKeyOffer { seller_id: seller.clone(), offer_id: *id });
                    match find_pre(changes, &key).map(|e| &e.data) {
                        Some(LedgerEntryData::Offer(o)) => {
                            t.price_n = o.price.n as i64;
                            t.price_d = o.price.d as i64;
                        }
                        _ => {
                            errs.push(format!("ledger {seq} op {op_id}: no pre-image for offer {id}"));
                            continue;
                        }
                    }
                }
                if let Some(pid) = &a.pool {
                    t.pool_id = Some(hex::encode(pid.0 .0));
                    let key = LedgerKey::LiquidityPool(stellar_xdr::LedgerKeyLiquidityPool { liquidity_pool_id: pid.clone() });
                    match find_pre(changes, &key) {
                        Some(pre) => t.rounding_slippage = pool_slippage(op_type, &a, pre),
                        None => errs.push(format!("ledger {seq} op {op_id}: no pre-image for pool {}", hex::encode(pid.0 .0))),
                    }
                }
                out.push(t);
            }
        }
    }
    (out, errs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toid_layout() {
        // Horizon toid.New(ledger, tx, op): ledger<<32 | tx<<12 | op
        assert_eq!(toid(1, 1, 1), (1i64 << 32) | (1 << 12) | 1);
        assert_eq!(toid(65_290_112, 3, 2) >> 32, 65_290_112);
    }
}
