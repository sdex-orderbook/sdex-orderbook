// SPDX-License-Identifier: Apache-2.0
//! Applying one `LedgerCloseMeta` to a [`BookState`].
//!
//! Apply order inside a ledger (stellar-core): fee processing of every
//! transaction; per transaction in `tx_processing` order: `tx_changes_before`,
//! operation changes, `tx_changes_after`; then `post_tx_apply_fee_processing`
//! (meta V2); then upgrade changes. Evictions only touch Soroban entries and
//! are ignored. Every STATE pre-image of an offer or pool is compared with the
//! reconstructed state, which checks the bootstrap and the replay together.

use crate::error::{Code, Error, Result};
use crate::state::{ApplyStats, BookState};
use crate::trades::{self, Trade};
use stellar_xdr::{LedgerCloseMeta, LedgerEntryChange, LedgerEntryChanges, LedgerHeaderHistoryEntry, TransactionMeta, TransactionResult};

/// One transaction in apply order. `index` is 1-based (Horizon TOID convention).
pub struct TxView<'a> {
    pub index: u32,
    pub result: &'a TransactionResult,
    pub fee_processing: &'a LedgerEntryChanges,
    pub meta: &'a TransactionMeta,
    pub post_fee: Option<&'a LedgerEntryChanges>,
}

pub struct LedgerView<'a> {
    pub header: &'a LedgerHeaderHistoryEntry,
    pub txs: Vec<TxView<'a>>,
    pub upgrades: Vec<&'a LedgerEntryChanges>,
}

impl LedgerView<'_> {
    pub fn sequence(&self) -> u32 {
        self.header.header.ledger_seq
    }
    pub fn close_time(&self) -> u64 {
        self.header.header.scp_value.close_time.0
    }
}

pub fn view(meta: &LedgerCloseMeta) -> LedgerView<'_> {
    match meta {
        LedgerCloseMeta::V0(m) => LedgerView {
            header: &m.ledger_header,
            txs: m
                .tx_processing
                .iter()
                .enumerate()
                .map(|(i, t)| TxView {
                    index: i as u32 + 1,
                    result: &t.result.result,
                    fee_processing: &t.fee_processing,
                    meta: &t.tx_apply_processing,
                    post_fee: None,
                })
                .collect(),
            upgrades: m.upgrades_processing.iter().map(|u| &u.changes).collect(),
        },
        LedgerCloseMeta::V1(m) => LedgerView {
            header: &m.ledger_header,
            txs: m
                .tx_processing
                .iter()
                .enumerate()
                .map(|(i, t)| TxView {
                    index: i as u32 + 1,
                    result: &t.result.result,
                    fee_processing: &t.fee_processing,
                    meta: &t.tx_apply_processing,
                    post_fee: None,
                })
                .collect(),
            upgrades: m.upgrades_processing.iter().map(|u| &u.changes).collect(),
        },
        LedgerCloseMeta::V2(m) => LedgerView {
            header: &m.ledger_header,
            txs: m
                .tx_processing
                .iter()
                .enumerate()
                .map(|(i, t)| TxView {
                    index: i as u32 + 1,
                    result: &t.result.result,
                    fee_processing: &t.fee_processing,
                    meta: &t.tx_apply_processing,
                    post_fee: Some(&t.post_tx_apply_fee_processing),
                })
                .collect(),
            upgrades: m.upgrades_processing.iter().map(|u| &u.changes).collect(),
        },
    }
}

/// (changes before ops, per-op changes, changes after ops) of one transaction.
pub fn tx_changes(meta: &TransactionMeta) -> (Vec<&LedgerEntryChanges>, Vec<&LedgerEntryChanges>, Vec<&LedgerEntryChanges>) {
    match meta {
        TransactionMeta::V0(ops) => (vec![], ops.iter().map(|o| &o.changes).collect(), vec![]),
        TransactionMeta::V1(v) => (vec![&v.tx_changes], v.operations.iter().map(|o| &o.changes).collect(), vec![]),
        TransactionMeta::V2(v) => {
            (vec![&v.tx_changes_before], v.operations.iter().map(|o| &o.changes).collect(), vec![&v.tx_changes_after])
        }
        TransactionMeta::V3(v) => {
            (vec![&v.tx_changes_before], v.operations.iter().map(|o| &o.changes).collect(), vec![&v.tx_changes_after])
        }
        TransactionMeta::V4(v) => {
            (vec![&v.tx_changes_before], v.operations.iter().map(|o| &o.changes).collect(), vec![&v.tx_changes_after])
        }
    }
}

fn apply_changes(state: &mut BookState, changes: &LedgerEntryChanges, stats: &mut ApplyStats, strict: bool) -> Result<()> {
    for c in changes.iter() {
        match c {
            LedgerEntryChange::Created(e) | LedgerEntryChange::Updated(e) | LedgerEntryChange::Restored(e) => state.upsert(e, stats)?,
            LedgerEntryChange::Removed(k) => match state.remove(k, stats) {
                Ok(()) => {}
                Err(e) if !strict => {
                    stats.state_mismatch += 1;
                    let _ = e;
                }
                Err(e) => return Err(e),
            },
            LedgerEntryChange::State(e) => match state.check_state(e) {
                Some(true) => stats.state_checked += 1,
                Some(false) => {
                    stats.state_mismatch += 1;
                    if strict {
                        return Err(Error::new(
                            Code::Consistency,
                            format!("STATE pre-image mismatch at ledger {}: {:?}", state.ledger + 1, e.to_key()),
                        ));
                    }
                }
                None => {}
            },
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub sequence: u32,
    pub close_time: u64,
    pub stats: ApplyStats,
    pub trades: Vec<Trade>,
    pub trade_errors: Vec<String>,
}

/// Apply one ledger. `strict` turns any pre-image mismatch into an error
/// (default in tests); the CLI counts mismatches and reports them instead.
pub fn apply(state: &mut BookState, meta: &LedgerCloseMeta, strict: bool) -> Result<Outcome> {
    let v = view(meta);
    let seq = v.sequence();
    if state.ledger != 0 && seq != state.ledger + 1 {
        return Err(Error::new(Code::Sequence, format!("state is at ledger {}, got ledger {seq}", state.ledger)));
    }
    let mut stats = ApplyStats::default();
    // Trades read only results and per-op pre-images, so derive them first.
    let (trades, trade_errors) = trades::from_view(&v);
    for t in &v.txs {
        apply_changes(state, t.fee_processing, &mut stats, strict)?;
    }
    for t in &v.txs {
        let (before, ops, after) = tx_changes(t.meta);
        for c in before.into_iter().chain(ops).chain(after) {
            apply_changes(state, c, &mut stats, strict)?;
        }
    }
    for t in &v.txs {
        if let Some(p) = t.post_fee {
            apply_changes(state, p, &mut stats, strict)?;
        }
    }
    for u in &v.upgrades {
        apply_changes(state, u, &mut stats, strict)?;
    }
    state.ledger = seq;
    state.close_time = v.close_time();
    Ok(Outcome { sequence: seq, close_time: v.close_time(), stats, trades, trade_errors })
}
