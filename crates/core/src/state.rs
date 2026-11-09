// SPDX-License-Identifier: Apache-2.0
//! In-memory SDEX state: every offer and every classic constant-product pool at
//! one ledger, indexed the way the order-book and path queries need.
//!
//! * `books[(selling, buying)]` holds offer keys sorted by price (cheapest
//!   first), then by insertion sequence. Horizon's graph uses the same order
//!   (`edges.go addOffer`: an update is a remove + add, so it moves behind
//!   offers of equal price).
//! * Adjacency sets give the path finder its edges in a deterministic order
//!   (ascending interned asset id). Horizon iterates in edge-insertion order;
//!   SPEC section 5.4 lists that as a known source of tie differences.

use crate::asset;
use crate::error::{Code, Error, Result};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use stellar_xdr::{LedgerEntry, LedgerEntryData, LedgerKey, LiquidityPoolEntryBody};

pub type AssetIdx = u32;

#[derive(Debug, Default, Clone)]
pub struct Assets {
    names: Vec<String>,
    index: HashMap<String, AssetIdx>,
}

impl Assets {
    pub fn intern(&mut self, name: &str) -> AssetIdx {
        if let Some(i) = self.index.get(name) {
            return *i;
        }
        let i = self.names.len() as AssetIdx;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        i
    }
    pub fn get(&self, name: &str) -> Option<AssetIdx> {
        self.index.get(name).copied()
    }
    pub fn name(&self, i: AssetIdx) -> &str {
        &self.names[i as usize]
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct OfferRec {
    pub entry: LedgerEntry,
    pub id: i64,
    pub seller: [u8; 32],
    pub selling: AssetIdx,
    pub buying: AssetIdx,
    pub amount: i64,
    pub n: i32,
    pub d: i32,
    pub seq: u64,
}

#[derive(Debug, Clone)]
pub struct PoolRec {
    pub entry: LedgerEntry,
    pub id: [u8; 32],
    pub a: AssetIdx,
    pub b: AssetIdx,
    pub fee: i32,
    pub reserve_a: i64,
    pub reserve_b: i64,
    pub shares: i64,
}

/// Sort key inside one book: price ascending, then insertion order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferKey {
    pub n: i32,
    pub d: i32,
    pub seq: u64,
    pub id: i64,
}

impl Ord for OfferKey {
    fn cmp(&self, o: &Self) -> Ordering {
        crate::price::cmp(self.n as i64, self.d as i64, o.n as i64, o.d as i64).then(self.seq.cmp(&o.seq)).then(self.id.cmp(&o.id))
    }
}
impl PartialOrd for OfferKey {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ApplyStats {
    pub offers_upserted: u64,
    pub offers_removed: u64,
    pub pools_upserted: u64,
    pub pools_removed: u64,
    /// STATE pre-images of offers/pools that matched the reconstructed state.
    pub state_checked: u64,
    /// STATE pre-images that did not match (should stay 0).
    pub state_mismatch: u64,
}

impl ApplyStats {
    pub fn add(&mut self, o: &ApplyStats) {
        self.offers_upserted += o.offers_upserted;
        self.offers_removed += o.offers_removed;
        self.pools_upserted += o.pools_upserted;
        self.pools_removed += o.pools_removed;
        self.state_checked += o.state_checked;
        self.state_mismatch += o.state_mismatch;
    }
}

#[derive(Debug, Default, Clone)]
pub struct BookState {
    /// Ledger whose post-state this is.
    pub ledger: u32,
    /// Close time (unix seconds) of `ledger`, 0 if unknown (bucket bootstrap).
    pub close_time: u64,
    pub assets: Assets,
    offers: HashMap<i64, OfferRec>,
    books: HashMap<(AssetIdx, AssetIdx), BTreeSet<OfferKey>>,
    sell_adj: HashMap<AssetIdx, BTreeSet<AssetIdx>>,
    buy_adj: HashMap<AssetIdx, BTreeSet<AssetIdx>>,
    pools: HashMap<[u8; 32], PoolRec>,
    pool_by_pair: HashMap<(AssetIdx, AssetIdx), [u8; 32]>,
    pool_adj: HashMap<AssetIdx, BTreeSet<AssetIdx>>,
    next_seq: u64,
}

fn pair_key(a: AssetIdx, b: AssetIdx) -> (AssetIdx, AssetIdx) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

impl BookState {
    pub fn new() -> BookState {
        BookState::default()
    }

    pub fn offer_count(&self) -> usize {
        self.offers.len()
    }
    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }
    pub fn offer(&self, id: i64) -> Option<&OfferRec> {
        self.offers.get(&id)
    }
    pub fn pool(&self, id: &[u8; 32]) -> Option<&PoolRec> {
        self.pools.get(id)
    }

    /// Offers sorted by id (deterministic output order).
    pub fn offers_sorted(&self) -> Vec<&OfferRec> {
        let mut v: Vec<&OfferRec> = self.offers.values().collect();
        v.sort_by_key(|o| o.id);
        v
    }
    pub fn pools_sorted(&self) -> Vec<&PoolRec> {
        let mut v: Vec<&PoolRec> = self.pools.values().collect();
        v.sort_by_key(|p| p.id);
        v
    }

    /// Offers selling `selling` for `buying`, cheapest first.
    pub fn book(&self, selling: AssetIdx, buying: AssetIdx) -> impl Iterator<Item = &OfferRec> + '_ {
        self.books.get(&(selling, buying)).into_iter().flat_map(|s| s.iter()).map(move |k| &self.offers[&k.id])
    }

    pub fn has_book(&self, selling: AssetIdx, buying: AssetIdx) -> bool {
        self.books.contains_key(&(selling, buying))
    }

    /// Assets `x` for which some offer sells `asset` and buys `x`.
    pub fn sell_neighbors(&self, asset: AssetIdx) -> impl Iterator<Item = AssetIdx> + '_ {
        self.sell_adj.get(&asset).into_iter().flat_map(|s| s.iter().copied())
    }
    /// Assets `x` for which some offer buys `asset` and sells `x`.
    pub fn buy_neighbors(&self, asset: AssetIdx) -> impl Iterator<Item = AssetIdx> + '_ {
        self.buy_adj.get(&asset).into_iter().flat_map(|s| s.iter().copied())
    }
    pub fn pool_neighbors(&self, asset: AssetIdx) -> impl Iterator<Item = AssetIdx> + '_ {
        self.pool_adj.get(&asset).into_iter().flat_map(|s| s.iter().copied())
    }
    pub fn pool_for(&self, a: AssetIdx, b: AssetIdx) -> Option<&PoolRec> {
        self.pool_by_pair.get(&pair_key(a, b)).map(|id| &self.pools[id])
    }

    /// Insert or replace an offer or pool entry. Other entry types are ignored.
    pub fn upsert(&mut self, entry: &LedgerEntry, stats: &mut ApplyStats) -> Result<()> {
        match &entry.data {
            LedgerEntryData::Offer(o) => {
                self.remove_offer(o.offer_id);
                let selling = self.assets.intern(&asset::from_xdr(&o.selling));
                let buying = self.assets.intern(&asset::from_xdr(&o.buying));
                if o.price.n <= 0 || o.price.d <= 0 {
                    return Err(Error::new(Code::Consistency, format!("offer {} has non-positive price", o.offer_id)));
                }
                let seq = self.next_seq;
                self.next_seq += 1;
                let rec = OfferRec {
                    entry: entry.clone(),
                    id: o.offer_id,
                    seller: asset::account_bytes(&o.seller_id),
                    selling,
                    buying,
                    amount: o.amount,
                    n: o.price.n,
                    d: o.price.d,
                    seq,
                };
                self.books.entry((selling, buying)).or_default().insert(OfferKey { n: rec.n, d: rec.d, seq, id: rec.id });
                self.sell_adj.entry(selling).or_default().insert(buying);
                self.buy_adj.entry(buying).or_default().insert(selling);
                self.offers.insert(rec.id, rec);
                stats.offers_upserted += 1;
            }
            LedgerEntryData::LiquidityPool(p) => {
                let LiquidityPoolEntryBody::LiquidityPoolConstantProduct(cp) = &p.body;
                let id = p.liquidity_pool_id.0 .0;
                let a = self.assets.intern(&asset::from_xdr(&cp.params.asset_a));
                let b = self.assets.intern(&asset::from_xdr(&cp.params.asset_b));
                let rec = PoolRec {
                    entry: entry.clone(),
                    id,
                    a,
                    b,
                    fee: cp.params.fee,
                    reserve_a: cp.reserve_a,
                    reserve_b: cp.reserve_b,
                    shares: cp.total_pool_shares,
                };
                self.pool_by_pair.insert(pair_key(a, b), id);
                self.pool_adj.entry(a).or_default().insert(b);
                self.pool_adj.entry(b).or_default().insert(a);
                self.pools.insert(id, rec);
                stats.pools_upserted += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn remove_offer(&mut self, id: i64) -> bool {
        let Some(rec) = self.offers.remove(&id) else { return false };
        let key = (rec.selling, rec.buying);
        if let Some(set) = self.books.get_mut(&key) {
            set.remove(&OfferKey { n: rec.n, d: rec.d, seq: rec.seq, id });
            if set.is_empty() {
                self.books.remove(&key);
                if let Some(s) = self.sell_adj.get_mut(&rec.selling) {
                    s.remove(&rec.buying);
                }
                if let Some(s) = self.buy_adj.get_mut(&rec.buying) {
                    s.remove(&rec.selling);
                }
            }
        }
        true
    }

    fn remove_pool(&mut self, id: &[u8; 32]) -> bool {
        let Some(rec) = self.pools.remove(id) else { return false };
        self.pool_by_pair.remove(&pair_key(rec.a, rec.b));
        if let Some(s) = self.pool_adj.get_mut(&rec.a) {
            s.remove(&rec.b);
        }
        if let Some(s) = self.pool_adj.get_mut(&rec.b) {
            s.remove(&rec.a);
        }
        true
    }

    /// Apply a REMOVED change. Unknown keys of tracked types are a consistency error.
    pub fn remove(&mut self, key: &LedgerKey, stats: &mut ApplyStats) -> Result<()> {
        match key {
            LedgerKey::Offer(k) => {
                if !self.remove_offer(k.offer_id) {
                    return Err(Error::new(Code::Consistency, format!("remove of unknown offer {}", k.offer_id)));
                }
                stats.offers_removed += 1;
            }
            LedgerKey::LiquidityPool(k) => {
                if !self.remove_pool(&k.liquidity_pool_id.0 .0) {
                    return Err(Error::new(Code::Consistency, format!("remove of unknown pool {}", hex::encode(k.liquidity_pool_id.0 .0))));
                }
                stats.pools_removed += 1;
            }
            _ => {}
        }
        Ok(())
    }

    /// Compare a STATE pre-image with what we hold. Returns `None` for untracked types.
    pub fn check_state(&self, entry: &LedgerEntry) -> Option<bool> {
        match &entry.data {
            LedgerEntryData::Offer(o) => Some(self.offers.get(&o.offer_id).is_some_and(|r| &r.entry == entry)),
            LedgerEntryData::LiquidityPool(p) => Some(self.pools.get(&p.liquidity_pool_id.0 .0).is_some_and(|r| &r.entry == entry)),
            _ => None,
        }
    }

    /// Total number of interned assets (path-search array size).
    pub fn total_assets(&self) -> usize {
        self.assets.len()
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Builders for synthetic entries used by unit tests (labelled synthetic).
    use stellar_xdr::*;

    pub fn account(b: u8) -> AccountId {
        AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([b; 32])))
    }

    pub fn credit(code: &str, issuer: u8) -> Asset {
        let mut c = [0u8; 4];
        c[..code.len()].copy_from_slice(code.as_bytes());
        Asset::CreditAlphanum4(AlphaNum4 { asset_code: AssetCode4(c), issuer: account(issuer) })
    }

    pub fn offer(id: i64, seller: u8, selling: Asset, buying: Asset, amount: i64, n: i32, d: i32) -> LedgerEntry {
        LedgerEntry {
            last_modified_ledger_seq: 1,
            data: LedgerEntryData::Offer(OfferEntry {
                seller_id: account(seller),
                offer_id: id,
                selling,
                buying,
                amount,
                price: Price { n, d },
                flags: 0,
                ext: OfferEntryExt::V0,
            }),
            ext: LedgerEntryExt::V0,
        }
    }

    pub fn pool(id: u8, a: Asset, b: Asset, ra: i64, rb: i64) -> LedgerEntry {
        LedgerEntry {
            last_modified_ledger_seq: 1,
            data: LedgerEntryData::LiquidityPool(LiquidityPoolEntry {
                liquidity_pool_id: PoolId(Hash([id; 32])),
                body: LiquidityPoolEntryBody::LiquidityPoolConstantProduct(LiquidityPoolEntryConstantProduct {
                    params: LiquidityPoolConstantProductParameters { asset_a: a, asset_b: b, fee: 30 },
                    reserve_a: ra,
                    reserve_b: rb,
                    total_pool_shares: 1000,
                    pool_shares_trust_line_count: 1,
                }),
            }),
            ext: LedgerEntryExt::V0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use stellar_xdr::Asset;

    #[test]
    fn books_sorted_and_updates_move_to_back() {
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        s.upsert(&offer(1, 1, Asset::Native, usd.clone(), 100, 2, 1), &mut st).unwrap();
        s.upsert(&offer(2, 1, Asset::Native, usd.clone(), 100, 1, 1), &mut st).unwrap();
        s.upsert(&offer(3, 1, Asset::Native, usd.clone(), 100, 2, 2), &mut st).unwrap();
        let n = s.assets.get("native").unwrap();
        let u = s.assets.get(&crate::asset::from_xdr(&usd)).unwrap();
        let ids: Vec<i64> = s.book(n, u).map(|o| o.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
        // update offer 2: same price, moves behind 3
        s.upsert(&offer(2, 1, Asset::Native, usd.clone(), 50, 1, 1), &mut st).unwrap();
        let ids: Vec<i64> = s.book(n, u).map(|o| o.id).collect();
        assert_eq!(ids, vec![3, 2, 1]);
        assert_eq!(s.sell_neighbors(n).collect::<Vec<_>>(), vec![u]);
        for id in [1, 2, 3] {
            s.remove(&offer(id, 1, Asset::Native, usd.clone(), 1, 1, 1).to_key(), &mut st).unwrap();
        }
        assert_eq!(s.offer_count(), 0);
        assert_eq!(s.sell_neighbors(n).count(), 0);
        assert!(s.remove(&offer(1, 1, Asset::Native, usd, 1, 1, 1).to_key(), &mut st).is_err());
    }

    #[test]
    fn pools_and_state_checks() {
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        let p = pool(7, Asset::Native, usd.clone(), 1000, 2000);
        s.upsert(&p, &mut st).unwrap();
        let n = s.assets.get("native").unwrap();
        let u = s.assets.get(&crate::asset::from_xdr(&usd)).unwrap();
        assert_eq!(s.pool_for(u, n).unwrap().reserve_b, 2000);
        assert_eq!(s.check_state(&p), Some(true));
        assert_eq!(s.check_state(&pool(7, Asset::Native, usd.clone(), 1, 2)), Some(false));
        s.remove(&p.to_key(), &mut st).unwrap();
        assert!(s.pool_for(n, u).is_none());
        assert_eq!(st.pools_upserted, 1);
        assert_eq!(st.pools_removed, 1);
    }
}
