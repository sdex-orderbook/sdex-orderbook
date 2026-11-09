// SPDX-License-Identifier: Apache-2.0
//! Order-book summary with Horizon `/order_book` semantics.
//!
//! Horizon (`internal/db2/history/orderbook.go`) groups offers by the float64
//! column `price = N/D`, keeps the first `limit` levels per side in ascending
//! `price`, sums `amount` per level and prints the level's reduced rational as
//! `price_r` and `FloatString(7)` as `price`. Bids are the offers selling the
//! counter asset; their rational is inverted. Amounts are in the units the offer
//! sells (base for asks, counter for bids). Pools are not part of `/order_book`.

use crate::amount;
use crate::price::{float_string7, reduce};
use crate::state::BookState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub n: i128,
    pub d: i128,
    pub price: String,
    /// Sum of offer amounts, stroops.
    pub amount: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

fn levels(state: &BookState, selling: &str, buying: &str, limit: usize, invert: bool) -> Vec<Level> {
    let (Some(s), Some(b)) = (state.assets.get(selling), state.assets.get(buying)) else { return Vec::new() };
    let mut out: Vec<Level> = Vec::new();
    let mut last_f: Option<f64> = None;
    for o in state.book(s, b) {
        let f = o.n as f64 / o.d as f64;
        if last_f == Some(f) {
            if let Some(l) = out.last_mut() {
                l.amount += o.amount as i128;
            }
            continue;
        }
        if out.len() == limit {
            break;
        }
        last_f = Some(f);
        let (mut n, mut d) = reduce(o.n as i128, o.d as i128);
        if invert {
            std::mem::swap(&mut n, &mut d);
        }
        out.push(Level { n, d, price: float_string7(n, d), amount: o.amount as i128 });
    }
    out
}

/// Order book for `base` (selling) / `counter` (buying), `limit` levels per side.
pub fn summary(state: &BookState, base: &str, counter: &str, limit: usize) -> Summary {
    Summary { asks: levels(state, base, counter, limit, false), bids: levels(state, counter, base, limit, true) }
}

impl Level {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "price_r": {"n": self.n as i64, "d": self.d as i64},
            "price": self.price,
            "amount": amount::format(self.amount),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::*;
    use crate::state::ApplyStats;
    use stellar_xdr::Asset;

    #[test]
    fn groups_levels_and_inverts_bids() {
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        let u = crate::asset::from_xdr(&usd);
        // asks: sell native for USD at 1/5 and 2/10 (same level) and 1/4
        s.upsert(&offer(1, 1, Asset::Native, usd.clone(), 100, 1, 5), &mut st).unwrap();
        s.upsert(&offer(2, 2, Asset::Native, usd.clone(), 50, 2, 10), &mut st).unwrap();
        s.upsert(&offer(3, 2, Asset::Native, usd.clone(), 70, 1, 4), &mut st).unwrap();
        // bid: sell USD for native at 6/1 (= 1/6 USD per XLM)
        s.upsert(&offer(4, 3, usd.clone(), Asset::Native, 30, 6, 1), &mut st).unwrap();
        let sum = summary(&s, "native", &u, 20);
        assert_eq!(sum.asks.len(), 2);
        assert_eq!((sum.asks[0].n, sum.asks[0].d, sum.asks[0].amount), (1, 5, 150));
        assert_eq!(sum.asks[0].price, "0.2000000");
        assert_eq!(sum.asks[1].price, "0.2500000");
        assert_eq!((sum.bids[0].n, sum.bids[0].d, sum.bids[0].amount), (1, 6, 30));
        assert_eq!(sum.bids[0].price, "0.1666667");
        let one = summary(&s, "native", &u, 1);
        assert_eq!(one.asks.len(), 1);
        assert_eq!(one.asks[0].amount, 150);
        assert!(summary(&s, "native", "credit_alphanum4/NOPE/G", 5).asks.is_empty());
    }
}
