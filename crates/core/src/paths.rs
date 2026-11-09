// SPDX-License-Identifier: Apache-2.0
//! Strict-send and strict-receive path finding over offers and classic pools.
//!
//! This is a port of go-stellar-sdk `exp/orderbook/search.go` (`search`,
//! `processVenues`, `consumeOffersForSellingAsset/BuyingAsset`) and
//! `graph.go` (`FindPaths`, `FindFixedPaths`, `sortAndFilterPaths`), the
//! engine behind Horizon `/paths/*` (Apache-2.0, see NOTICE).
//!
//! The search is a bounded Bellman-Ford relaxation: for `max_path_length`
//! rounds, every asset with a best amount relaxes its edges; each edge is
//! either the sorted offers of one book or a pool, and the better of the two
//! is taken (offers and the pool are never split inside one hop). Every
//! improvement that lands on a target asset is recorded as a path. Results
//! are sorted by asset string, then amount, then path length, and capped at
//! `max_paths_per_asset` per asset (Horizon uses 5).
//!
//! Differences from Horizon are listed in SPEC section 5.4.

use crate::error::{Code, Error, Result};
use crate::pool_math::{calculate_pool_expectation, calculate_pool_payout};
use crate::price::{convert_to_buying_units, mul_fraction_round_down, PriceError};
use crate::state::{AssetIdx, BookState, PoolRec};

/// Horizon `simplepath.maxAssetsPerPath`.
pub const MAX_PATHS_PER_ASSET: usize = 5;
/// Horizon `--max-path-length` default.
pub const DEFAULT_MAX_PATH_LENGTH: usize = 3;
/// Horizon `simplepath.MaxInMemoryPathLength`.
pub const MAX_PATH_LENGTH: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub source_asset: String,
    pub source_amount: i64,
    pub destination_asset: String,
    pub destination_amount: i64,
    /// Interior assets in payment order (source side first).
    pub path: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub max_path_length: usize,
    pub include_pools: bool,
    pub max_paths_per_asset: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options { max_path_length: DEFAULT_MAX_PATH_LENGTH, include_pools: true, max_paths_per_asset: MAX_PATHS_PER_ASSET }
    }
}

fn arith(e: PriceError) -> Error {
    Error::new(Code::Arithmetic, format!("{e:?}"))
}

/// Shared search-state behaviour (Go `searchState`).
trait SearchState {
    fn is_terminal(&self, asset: AssetIdx) -> bool;
    fn include_path(&self, asset: AssetIdx, amount: i64) -> bool;
    fn better(&self, current: i64, alternative: i64) -> bool;
    fn append(&mut self, path: Vec<AssetIdx>, asset: AssetIdx, amount: i64);
    /// Neighbours of `asset` (offer books and pools) in deterministic order.
    fn neighbors(&self, asset: AssetIdx) -> Vec<AssetIdx>;
    /// Amount after trading across one hop, or <= 0 if impossible.
    fn process_venues(&self, current: AssetIdx, next: AssetIdx, amount: i64) -> Result<i64>;
}

fn merged_neighbors(a: impl Iterator<Item = AssetIdx>, b: impl Iterator<Item = AssetIdx>) -> Vec<AssetIdx> {
    let mut v: Vec<AssetIdx> = a.chain(b).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[derive(Clone, Copy)]
struct Node {
    asset: AssetIdx,
    prev: Option<usize>,
}

fn node_path(arena: &[Node], mut i: Option<usize>) -> Vec<AssetIdx> {
    let mut out = Vec::with_capacity(8);
    while let Some(x) = i {
        out.push(arena[x].asset);
        i = arena[x].prev;
    }
    out.reverse();
    out
}

fn node_contains(arena: &[Node], mut i: Option<usize>, asset: AssetIdx) -> bool {
    while let Some(x) = i {
        if arena[x].asset == asset {
            return true;
        }
        i = arena[x].prev;
    }
    false
}

/// Go `search`.
fn search<S: SearchState>(state: &mut S, total: usize, max_len: usize, source: AssetIdx, source_amount: i64) -> Result<()> {
    let mut best_amount = vec![0i64; total];
    let mut update_amount = vec![0i64; total];
    let mut best_path: Vec<Option<usize>> = vec![None; total];
    let mut update_path: Vec<Option<usize>> = vec![None; total];
    let mut arena: Vec<Node> = Vec::with_capacity(total);
    let s = source as usize;
    best_amount[s] = source_amount;
    update_amount[s] = source_amount;
    arena.push(Node { asset: source, prev: None });
    best_path[s] = Some(0);
    if state.include_path(source, source_amount) {
        state.append(vec![source], source, source_amount);
    }
    let mut updated: Vec<AssetIdx> = Vec::new();
    for i in 0..max_len {
        updated.clear();
        for cur in 0..total {
            let cur_amount = best_amount[cur];
            if cur_amount == 0 {
                continue;
            }
            let path_to_cur = best_path[cur];
            for next in state.neighbors(cur as AssetIdx) {
                if i == max_len - 1 && !state.is_terminal(next) {
                    continue;
                }
                if node_contains(&arena, path_to_cur, next) {
                    continue;
                }
                let next_amount = state.process_venues(cur as AssetIdx, next, cur_amount)?;
                if next_amount <= 0 {
                    continue;
                }
                let n = next as usize;
                if state.better(update_amount[n], next_amount) {
                    let new_entry = update_amount[n] == best_amount[n];
                    update_amount[n] = next_amount;
                    if new_entry {
                        updated.push(next);
                        arena.push(Node { asset: next, prev: path_to_cur });
                        update_path[n] = Some(arena.len() - 1);
                    } else if let Some(x) = update_path[n] {
                        arena[x].prev = path_to_cur;
                    }
                    if state.include_path(next, next_amount) {
                        let mut p = node_path(&arena, best_path[cur]);
                        p.push(next);
                        state.append(p, next, next_amount);
                    }
                }
            }
        }
        if i < max_len - 1 {
            for &a in &updated {
                best_path[a as usize] = update_path[a as usize];
                best_amount[a as usize] = update_amount[a as usize];
            }
        }
    }
    Ok(())
}

fn positive_min(a: i64, b: i64) -> i64 {
    if b <= 0 {
        return a;
    }
    if b < a || a <= 0 {
        return b;
    }
    a
}

/// Pool trade. `deposit = true`: put `amount` of `from` in, how much of the other
/// asset comes out. `deposit = false`: want `amount` of `to` out, how much of
/// `from` must go in. `None` when the pool cannot do it.
fn pool_trade(pool: &PoolRec, from: AssetIdx, amount: i64, deposit: bool) -> Option<i64> {
    let (x, y) = if pool.a == from { (pool.reserve_a, pool.reserve_b) } else { (pool.reserve_b, pool.reserve_a) };
    if amount <= 0 {
        return None;
    }
    if deposit {
        calculate_pool_payout(x, y, amount, pool.fee, false).map(|r| r.0)
    } else {
        calculate_pool_expectation(x, y, amount, pool.fee, false).map(|r| r.0)
    }
}

/// Strict receive (Go `sellingGraphSearchState`): search from the destination
/// back towards the source assets, minimising the source amount.
struct Selling<'a> {
    g: &'a BookState,
    dest: String,
    dest_amount: i64,
    ignore_offers_from: Option<[u8; 32]>,
    /// source asset -> balance
    targets: Vec<Option<i64>>,
    validate_balance: bool,
    include_pools: bool,
    out: Vec<Path>,
}

impl SearchState for Selling<'_> {
    fn is_terminal(&self, a: AssetIdx) -> bool {
        self.targets[a as usize].is_some()
    }
    fn include_path(&self, a: AssetIdx, amount: i64) -> bool {
        match self.targets[a as usize] {
            Some(bal) => !self.validate_balance || bal >= amount,
            None => false,
        }
    }
    fn better(&self, current: i64, alt: i64) -> bool {
        if current == 0 {
            return true;
        }
        if alt == 0 {
            return false;
        }
        alt < current
    }
    fn append(&mut self, path: Vec<AssetIdx>, asset: AssetIdx, amount: i64) {
        let interior: Vec<String> = if path.len() > 2 {
            path[1..path.len() - 1].iter().rev().map(|a| self.g.assets.name(*a).to_string()).collect()
        } else {
            Vec::new()
        };
        self.out.push(Path {
            source_asset: self.g.assets.name(asset).to_string(),
            source_amount: amount,
            destination_asset: self.dest.clone(),
            destination_amount: self.dest_amount,
            path: interior,
        });
    }
    fn neighbors(&self, a: AssetIdx) -> Vec<AssetIdx> {
        merged_neighbors(self.g.sell_neighbors(a), self.g.pool_neighbors(a))
    }
    fn process_venues(&self, cur: AssetIdx, next: AssetIdx, amount: i64) -> Result<i64> {
        // offers selling `cur` for `next`: how much `next` buys `amount` of `cur`
        let mut pool_amount = 0;
        if self.include_pools {
            if let Some(p) = self.g.pool_for(cur, next) {
                pool_amount = pool_trade(p, next, amount, false).unwrap_or(0);
            }
        }
        let has_offers = self.g.has_book(cur, next);
        if pool_amount == 0 && !has_offers {
            return Ok(-1);
        }
        if !has_offers {
            return Ok(positive_min(pool_amount, 0));
        }
        let mut total: i64 = 0;
        let mut remaining = amount;
        let mut result: i64 = -1;
        for o in self.g.book(cur, next) {
            if self.ignore_offers_from == Some(o.seller) {
                continue;
            }
            let (buying, selling) = match convert_to_buying_units(o.amount, remaining, o.n as i64, o.d as i64) {
                Ok(v) => v,
                Err(PriceError::Overflow) => {
                    result = -1;
                    remaining = -1;
                    break;
                }
                Err(e) if pool_amount == 0 => return Err(arith(e)),
                Err(_) => {
                    remaining = -1;
                    break;
                }
            };
            // Go's xdr.Int64 addition wraps; mirror it (a wrapped, negative total
            // makes the hop unusable, exactly as in Horizon).
            total = total.wrapping_add(buying);
            remaining -= selling;
            if remaining == 0 {
                result = total;
                break;
            }
            if remaining < 0 {
                if pool_amount == 0 {
                    return Err(Error::new(Code::Arithmetic, "sold too much"));
                }
                break;
            }
        }
        let _ = remaining;
        Ok(positive_min(pool_amount, result))
    }
}

/// Strict send (Go `buyingGraphSearchState`): search from the source asset
/// forwards, maximising the destination amount.
struct Buying<'a> {
    g: &'a BookState,
    source: String,
    source_amount: i64,
    targets: Vec<bool>,
    include_pools: bool,
    out: Vec<Path>,
}

impl SearchState for Buying<'_> {
    fn is_terminal(&self, a: AssetIdx) -> bool {
        self.targets[a as usize]
    }
    fn include_path(&self, a: AssetIdx, _amount: i64) -> bool {
        self.targets[a as usize]
    }
    fn better(&self, current: i64, alt: i64) -> bool {
        alt > current
    }
    fn append(&mut self, path: Vec<AssetIdx>, asset: AssetIdx, amount: i64) {
        let interior: Vec<String> =
            if path.len() > 2 { path[1..path.len() - 1].iter().map(|a| self.g.assets.name(*a).to_string()).collect() } else { Vec::new() };
        self.out.push(Path {
            source_asset: self.source.clone(),
            source_amount: self.source_amount,
            destination_asset: self.g.assets.name(asset).to_string(),
            destination_amount: amount,
            path: interior,
        });
    }
    fn neighbors(&self, a: AssetIdx) -> Vec<AssetIdx> {
        merged_neighbors(self.g.buy_neighbors(a), self.g.pool_neighbors(a))
    }
    fn process_venues(&self, cur: AssetIdx, next: AssetIdx, amount: i64) -> Result<i64> {
        // offers selling `next` and buying `cur`: spend `amount` of `cur`
        let mut pool_amount = 0;
        if self.include_pools {
            if let Some(p) = self.g.pool_for(cur, next) {
                pool_amount = pool_trade(p, cur, amount, true).unwrap_or(0);
            }
        }
        let has_offers = self.g.has_book(next, cur);
        if pool_amount == 0 && !has_offers {
            return Ok(-1);
        }
        if !has_offers {
            return Ok(pool_amount.max(0));
        }
        let mut total: i64 = 0;
        let mut remaining = amount;
        let mut result: i64 = -1;
        for o in self.g.book(next, cur) {
            let (n, d) = (o.n as i64, o.d as i64);
            match mul_fraction_round_down(remaining, d, n) {
                Ok(sold) => {
                    if sold == 0 {
                        result = -1;
                        break;
                    }
                    if sold <= o.amount {
                        result = total.wrapping_add(sold);
                        break;
                    }
                }
                Err(PriceError::Overflow) => {}
                Err(e) => {
                    if pool_amount == 0 {
                        return Err(arith(e));
                    }
                    break;
                }
            }
            let (buying, selling) = match convert_to_buying_units(o.amount, o.amount, n, d) {
                Ok(v) => v,
                Err(PriceError::Overflow) => {
                    result = -1;
                    break;
                }
                Err(e) => {
                    if pool_amount == 0 {
                        return Err(arith(e));
                    }
                    break;
                }
            };
            total = total.wrapping_add(selling);
            remaining -= buying;
            if remaining == 0 {
                result = total;
                break;
            }
            if remaining < 0 {
                if pool_amount == 0 {
                    return Err(Error::new(Code::Arithmetic, "sold too much"));
                }
                break;
            }
        }
        Ok(result.max(pool_amount))
    }
}

fn check_opts(o: &Options) -> Result<()> {
    if o.max_path_length == 0 || o.max_path_length > MAX_PATH_LENGTH {
        return Err(Error::bad(format!("max path length must be 1..={MAX_PATH_LENGTH}")));
    }
    Ok(())
}

/// Go `sortAndFilterPaths` (stable sort here; Go's `sort.Sort` is not stable).
fn sort_and_filter(mut paths: Vec<Path>, max_per_asset: usize, by_source: bool) -> Vec<Path> {
    paths.sort_by(|a, b| {
        if by_source {
            a.source_asset.cmp(&b.source_asset).then(a.source_amount.cmp(&b.source_amount)).then(a.path.len().cmp(&b.path.len()))
        } else {
            a.destination_asset
                .cmp(&b.destination_asset)
                .then(b.destination_amount.cmp(&a.destination_amount))
                .then(a.path.len().cmp(&b.path.len()))
        }
    });
    let mut out: Vec<Path> = Vec::with_capacity(paths.len());
    let mut count = 0;
    for p in paths {
        let same =
            out.last().is_some_and(
                |l: &Path| if by_source { l.source_asset == p.source_asset } else { l.destination_asset == p.destination_asset },
            );
        if !same {
            count = 1;
            out.push(p);
        } else if count < max_per_asset {
            count += 1;
            out.push(p);
        }
    }
    out
}

/// Horizon `/paths/strict-receive`. `sources` are (asset, balance); balances are
/// only checked when `validate_balance` (Horizon sets it for `source_account`).
pub fn strict_receive(
    g: &BookState,
    dest: &str,
    dest_amount: i64,
    sources: &[(String, i64)],
    validate_balance: bool,
    ignore_offers_from: Option<[u8; 32]>,
    opts: Options,
) -> Result<Vec<Path>> {
    check_opts(&opts)?;
    let total = g.total_assets();
    let mut targets = vec![None; total];
    let mut any = false;
    for (a, bal) in sources {
        if let Some(i) = g.assets.get(a) {
            targets[i as usize] = Some(*bal);
            any = true;
        }
    }
    let Some(d) = g.assets.get(dest) else { return Ok(Vec::new()) };
    if !any {
        return Ok(Vec::new());
    }
    let mut st = Selling {
        g,
        dest: dest.to_string(),
        dest_amount,
        ignore_offers_from,
        targets,
        validate_balance,
        include_pools: opts.include_pools,
        out: Vec::new(),
    };
    search(&mut st, total, opts.max_path_length, d, dest_amount)?;
    Ok(sort_and_filter(st.out, opts.max_paths_per_asset, true))
}

/// Horizon `/paths/strict-send`.
pub fn strict_send(g: &BookState, source: &str, source_amount: i64, dests: &[String], opts: Options) -> Result<Vec<Path>> {
    check_opts(&opts)?;
    let total = g.total_assets();
    let mut targets = vec![false; total];
    let mut any = false;
    for a in dests {
        if let Some(i) = g.assets.get(a) {
            targets[i as usize] = true;
            any = true;
        }
    }
    let Some(s) = g.assets.get(source) else { return Ok(Vec::new()) };
    if !any {
        return Ok(Vec::new());
    }
    let mut st = Buying { g, source: source.to_string(), source_amount, targets, include_pools: opts.include_pools, out: Vec::new() };
    search(&mut st, total, opts.max_path_length, s, source_amount)?;
    Ok(sort_and_filter(st.out, opts.max_paths_per_asset, false))
}

impl Path {
    pub fn to_json(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        crate::asset::json_fields(&self.source_asset, "source_", &mut m);
        m.insert("source_amount".into(), crate::amount::format(self.source_amount as i128).into());
        crate::asset::json_fields(&self.destination_asset, "destination_", &mut m);
        m.insert("destination_amount".into(), crate::amount::format(self.destination_amount as i128).into());
        m.insert("path".into(), serde_json::Value::Array(self.path.iter().map(|a| crate::asset::json(a)).collect()));
        serde_json::Value::Object(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::*;
    use crate::state::ApplyStats;
    use stellar_xdr::Asset;

    fn graph() -> (BookState, String, String, String) {
        // Synthetic graph: XLM->USD book, USD->EUR book, XLM/EUR pool.
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        let eur = credit("EUR", 8);
        // offer sells USD for XLM at 5 XLM per USD (n/d = 5/1), 1000 USD
        s.upsert(&offer(1, 1, usd.clone(), Asset::Native, 10_000_000_000, 5, 1), &mut st).unwrap();
        // offer sells EUR for USD at 1.25 USD per EUR, 500 EUR
        s.upsert(&offer(2, 2, eur.clone(), usd.clone(), 500_0000000, 5, 4), &mut st).unwrap();
        // pool XLM/EUR, 1,000,000 XLM : 100,000 EUR
        s.upsert(&pool(3, Asset::Native, eur.clone(), 10_000_000_000_000, 1_000_000_000_000), &mut st).unwrap();
        (s, "native".into(), crate::asset::from_xdr(&usd), crate::asset::from_xdr(&eur))
    }

    #[test]
    fn strict_send_direct_and_two_hop() {
        let (g, xlm, usd, eur) = graph();
        // spend 50 XLM -> USD: 10 USD
        let p = strict_send(&g, &xlm, 50_0000000, std::slice::from_ref(&usd), Options::default()).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].destination_amount, 10_0000000);
        assert!(p[0].path.is_empty());
        // spend 50 XLM -> EUR: via pool directly, or XLM->USD->EUR = 10 USD -> 8 EUR
        let p = strict_send(&g, &xlm, 50_0000000, std::slice::from_ref(&eur), Options::default()).unwrap();
        assert_eq!(p[0].destination_asset, eur);
        let direct = calculate_pool_payout(10_000_000_000_000, 1_000_000_000_000, 50_0000000, 30, false).unwrap().0;
        // pool gives ~4.98 EUR, the two-hop book route 8 EUR: best first
        assert_eq!(p[0].destination_amount, 8_0000000);
        assert_eq!(p[0].path, vec![usd.clone()]);
        assert!(p.iter().any(|x| x.destination_amount == direct && x.path.is_empty()));
        let no_pools =
            strict_send(&g, &xlm, 50_0000000, std::slice::from_ref(&eur), Options { include_pools: false, ..Options::default() }).unwrap();
        assert!(no_pools.iter().all(|x| !x.path.is_empty()));
    }

    #[test]
    fn strict_receive_direct_and_balance() {
        let (g, xlm, usd, eur) = graph();
        // receive 8 EUR paying XLM: book route costs 50 XLM (8 EUR = 10 USD = 50 XLM)
        let p = strict_receive(&g, &eur, 8_0000000, &[(xlm.clone(), 0)], false, None, Options::default()).unwrap();
        assert!(!p.is_empty());
        assert_eq!(p[0].source_asset, xlm);
        let via_pool = calculate_pool_expectation(10_000_000_000_000, 1_000_000_000_000, 8_0000000, 30, false).unwrap().0;
        assert_eq!(p[0].source_amount, 50_0000000.min(via_pool));
        // validate_balance with a too-small balance drops everything
        let p = strict_receive(&g, &eur, 8_0000000, &[(xlm.clone(), 1)], true, None, Options::default()).unwrap();
        assert!(p.is_empty());
        // ignoring the USD seller kills the book route
        let p = strict_receive(&g, &usd, 1_0000000, &[(xlm.clone(), 0)], false, Some([1; 32]), Options::default()).unwrap();
        assert!(p.iter().all(|x| !x.path.is_empty()));
    }

    #[test]
    fn same_asset_is_a_zero_hop_path_and_limits_apply() {
        let (g, xlm, _, _) = graph();
        let p = strict_send(&g, &xlm, 7, std::slice::from_ref(&xlm), Options::default()).unwrap();
        assert_eq!(p[0].destination_amount, 7);
        assert!(strict_send(&g, &xlm, 7, std::slice::from_ref(&xlm), Options { max_path_length: 6, ..Options::default() }).is_err());
        assert!(strict_send(&g, "credit_alphanum4/NOPE/G", 7, &[xlm], Options::default()).unwrap().is_empty());
    }

    #[test]
    fn overflowing_totals_wrap_like_go_instead_of_panicking() {
        // Two offers each costing ~6.4e18 units: the Go total wraps negative and the
        // hop is dropped. Found on mainnet data by `sdx diff` on 2026-11-04.
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        s.upsert(&offer(1, 1, usd.clone(), Asset::Native, 3_000_000_000, i32::MAX, 1), &mut st).unwrap();
        s.upsert(&offer(2, 2, usd.clone(), Asset::Native, 3_000_000_000, i32::MAX, 1), &mut st).unwrap();
        let u = crate::asset::from_xdr(&usd);
        let p = strict_receive(&s, &u, 6_000_000_000, &[("native".into(), 0)], false, None, Options::default()).unwrap();
        assert!(p.is_empty());
    }

    #[test]
    fn sort_and_filter_caps_per_asset() {
        let mk = |a: &str, amt: i64, hops: usize| Path {
            source_asset: a.into(),
            source_amount: amt,
            destination_asset: "native".into(),
            destination_amount: 1,
            path: vec!["x".into(); hops],
        };
        let v = vec![mk("b", 5, 0), mk("a", 3, 1), mk("a", 3, 0), mk("a", 1, 2), mk("a", 9, 0)];
        let out = sort_and_filter(v, 2, true);
        let got: Vec<(String, i64, usize)> = out.iter().map(|p| (p.source_asset.clone(), p.source_amount, p.path.len())).collect();
        assert_eq!(got, vec![("a".into(), 1, 2), ("a".into(), 3, 0), ("b".into(), 5, 0)]);
    }
}
