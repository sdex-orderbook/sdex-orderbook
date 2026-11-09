// SPDX-License-Identifier: Apache-2.0
//! OHLCV trade aggregations with Horizon `/trade_aggregations` semantics
//! (`internal/db2/history/trade_aggregation.go`):
//!
//! 1. Trades are first rolled up into 1-minute rows per (base, counter) in a
//!    canonical orientation (Horizon: `history_trades_60000`); open/close are
//!    the first/last trade by (op id, order); high/low compare rationals and on
//!    a tie keep the later trade (Postgres `max_price_agg`).
//! 2. A query in the other orientation swaps volumes and inverts prices.
//! 3. Bucket = ((ts - offset) / resolution) * resolution + offset. The page is
//!    the first `limit` buckets in `order` among minute rows with
//!    `start <= ts < end`; buckets are then rebuilt from all minute rows whose
//!    bucket lies in that range.
//! 4. `avg` = sum(counter) / sum(base) as Postgres numeric, printed as float64
//!    with 7 decimals; prices print as `FloatString(7)` of the raw rational.

use crate::error::{Error, Result};
use crate::price::{cmp, float_string7, pg_avg_string};
use crate::trades::Trade;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const ALLOWED_RESOLUTIONS: [i64; 6] = [60_000, 300_000, 900_000, 3_600_000, 86_400_000, 604_800_000];
/// Horizon `--rounding-slippage-filter` default.
pub const DEFAULT_SLIPPAGE_FILTER: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rat {
    pub n: i64,
    pub d: i64,
}

impl Rat {
    fn inv(self) -> Rat {
        Rat { n: self.d, d: self.n }
    }
    fn cmp(&self, o: &Rat) -> Ordering {
        cmp(self.n, self.d, o.n, o.d)
    }
    fn s(&self) -> String {
        float_string7(self.n as i128, self.d as i128)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Bucket start, unix ms.
    pub ts: i64,
    pub count: i64,
    pub base_volume: i128,
    pub counter_volume: i128,
    pub high: Rat,
    pub low: Rat,
    pub open: Rat,
    pub close: Rat,
    /// (op id, order) of the opening trade, used to order minute rows.
    pub open_key: (i64, u32),
}

impl Row {
    fn inverted(&self) -> Row {
        Row {
            ts: self.ts,
            count: self.count,
            base_volume: self.counter_volume,
            counter_volume: self.base_volume,
            high: self.low.inv(),
            low: self.high.inv(),
            open: self.open.inv(),
            close: self.close.inv(),
            open_key: self.open_key,
        }
    }

    fn merge(&mut self, o: &Row) {
        self.count += o.count;
        self.base_volume += o.base_volume;
        self.counter_volume += o.counter_volume;
        // Postgres max_price_agg: keep state only if strictly greater, else take the new value.
        if self.high.cmp(&o.high) != Ordering::Greater {
            self.high = o.high;
        }
        if self.low.cmp(&o.low) != Ordering::Less {
            self.low = o.low;
        }
        self.close = o.close;
    }

    pub fn to_json(&self) -> serde_json::Value {
        let r = |p: &Rat| serde_json::json!({"n": p.n.to_string(), "d": p.d.to_string()});
        serde_json::json!({
            "timestamp": self.ts.to_string(),
            "trade_count": self.count.to_string(),
            "base_volume": crate::amount::format(self.base_volume),
            "counter_volume": crate::amount::format(self.counter_volume),
            "avg": pg_avg_string(self.counter_volume, self.base_volume),
            "high": self.high.s(),
            "high_r": r(&self.high),
            "low": self.low.s(),
            "low_r": r(&self.low),
            "open": self.open.s(),
            "open_r": r(&self.open),
            "close": self.close.s(),
            "close_r": r(&self.close),
        })
    }
}

/// Minute rows per canonical pair `(a, b)` with `a < b` (string order);
/// rows are in (a = base, b = counter) orientation.
#[derive(Debug, Default, Clone)]
pub struct Minutes {
    rows: BTreeMap<(String, String), BTreeMap<i64, Row>>,
    /// Trades excluded by the rounding-slippage filter.
    pub filtered: u64,
    pub trades: u64,
}

impl Minutes {
    pub fn build(trades: &[Trade], slippage_filter: i64) -> Minutes {
        let mut m = Minutes::default();
        m.add(trades, slippage_filter);
        m
    }

    /// Add trades. Calls must come in ledger order (each call sorts its own
    /// trades by (op id, order)); incremental adds equal one `build`.
    pub fn add(&mut self, trades: &[Trade], slippage_filter: i64) {
        let mut sorted: Vec<&Trade> = trades.iter().collect();
        sorted.sort_by_key(|t| (t.op_id, t.order));
        for t in sorted {
            self.trades += 1;
            if t.rounding_slippage.unwrap_or(0) >= slippage_filter {
                self.filtered += 1;
                continue;
            }
            let ts = (t.close_time as i64 * 1000) / 60_000 * 60_000;
            let (key, base_v, counter_v, price) = if t.sold < t.bought {
                ((t.sold.clone(), t.bought.clone()), t.amount_sold, t.amount_bought, Rat { n: t.price_n, d: t.price_d })
            } else {
                ((t.bought.clone(), t.sold.clone()), t.amount_bought, t.amount_sold, Rat { n: t.price_d, d: t.price_n })
            };
            let row = Row {
                ts,
                count: 1,
                base_volume: base_v as i128,
                counter_volume: counter_v as i128,
                high: price,
                low: price,
                open: price,
                close: price,
                open_key: (t.op_id, t.order),
            };
            self.rows.entry(key).or_default().entry(ts).and_modify(|r| r.merge(&row)).or_insert(row);
        }
    }

    /// Pairs with at least one minute row, canonical orientation.
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str, usize)> + '_ {
        self.rows.iter().map(|((a, b), r)| (a.as_str(), b.as_str(), r.len()))
    }

    /// Minute rows for `base`/`counter` in that orientation, ascending by ts.
    pub fn oriented(&self, base: &str, counter: &str) -> Vec<Row> {
        if base < counter {
            self.rows.get(&(base.to_string(), counter.to_string())).map(|r| r.values().cloned().collect()).unwrap_or_default()
        } else {
            self.rows.get(&(counter.to_string(), base.to_string())).map(|r| r.values().map(Row::inverted).collect()).unwrap_or_default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Query {
    pub resolution: i64,
    pub offset: i64,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub descending: bool,
    pub limit: usize,
}

impl Query {
    /// Horizon's validation and start/end rounding (`GetTradeAggregationsQ`,
    /// `WithStartTime`, `WithEndTime`). Returns (start, end) after rounding.
    pub fn normalised(&self) -> Result<(i64, Option<i64>)> {
        if !ALLOWED_RESOLUTIONS.contains(&self.resolution) {
            return Err(Error::bad("resolution is not allowed"));
        }
        if self.offset % 3_600_000 != 0 || self.offset >= 86_400_000 || self.offset > self.resolution || self.offset < 0 {
            return Err(Error::bad("offset is not allowed."));
        }
        let (res, off) = (self.resolution, self.offset);
        let mut start = 0i64;
        let mut end = None;
        if let Some(e) = self.end_time {
            if e < off {
                return Err(Error::bad("end time is not allowed"));
            }
            end = Some((e - off) / res * res + off);
        }
        if let Some(s) = self.start_time {
            start = if s < off { off } else { (s - off + res - 1) / res * res + off };
            if let Some(e) = end {
                if start > e {
                    return Err(Error::bad("start time is not allowed"));
                }
            }
        }
        if let Some(e) = end {
            if e < start {
                return Err(Error::bad("end time is not allowed"));
            }
        }
        Ok((start, end))
    }
}

fn bucket(ts: i64, res: i64, off: i64) -> i64 {
    (ts - off) / res * res + off
}

/// Run a query over oriented minute rows.
pub fn aggregate(minutes: &[Row], q: &Query) -> Result<Vec<Row>> {
    let (start, end) = q.normalised()?;
    let (res, off) = (q.resolution, q.offset);
    // last_range_ts: distinct buckets of in-window minute rows, first `limit` in `order`.
    let mut buckets: Vec<i64> =
        minutes.iter().filter(|r| r.ts >= start && end.is_none_or(|e| r.ts < e)).map(|r| bucket(r.ts, res, off)).collect();
    buckets.sort_unstable();
    buckets.dedup();
    if q.descending {
        buckets.reverse();
    }
    buckets.truncate(q.limit);
    let (Some(lo), Some(hi)) = (buckets.iter().min().copied(), buckets.iter().max().copied()) else { return Ok(Vec::new()) };
    // raw_trades: minute rows whose bucket is inside [lo, hi], ordered ts, open op id.
    let mut raw: Vec<&Row> = minutes.iter().filter(|r| (lo..=hi).contains(&bucket(r.ts, res, off))).collect();
    raw.sort_by_key(|r| (r.ts, r.open_key));
    let mut out: BTreeMap<i64, Row> = BTreeMap::new();
    for r in raw {
        let b = bucket(r.ts, res, off);
        match out.get_mut(&b) {
            Some(acc) => acc.merge(r),
            None => {
                let mut x = r.clone();
                x.ts = b;
                out.insert(b, x);
            }
        }
    }
    let mut v: Vec<Row> = out.into_values().collect();
    if q.descending {
        v.reverse();
    }
    v.truncate(q.limit);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn trade(sec: u64, op: i64, sold: &str, bought: &str, a_sold: i64, a_bought: i64, n: i64, d: i64) -> Trade {
        Trade {
            ledger: 1,
            close_time: sec,
            op_id: op,
            order: 0,
            op_type: "manage_sell_offer".into(),
            sold: sold.into(),
            bought: bought.into(),
            amount_sold: a_sold,
            amount_bought: a_bought,
            price_n: n,
            price_d: d,
            offer_id: Some(1),
            seller: None,
            pool_id: None,
            rounding_slippage: None,
        }
    }

    fn q(res: i64, desc: bool, limit: usize) -> Query {
        Query { resolution: res, offset: 0, start_time: None, end_time: None, descending: desc, limit }
    }

    #[test]
    fn minute_rows_ohlc_and_orientation() {
        // "a" < "b": trades in both directions in the same minute.
        let t = vec![
            trade(60, 1, "a", "b", 100, 50, 1, 2), // price b/a = 1/2
            trade(70, 2, "b", "a", 30, 90, 3, 1),  // sells b for a at 3 a/b -> b/a = 1/3
            trade(100, 3, "a", "b", 10, 10, 1, 1), // 1/1
            trade(130, 4, "a", "b", 20, 20, 2, 2), // next minute
        ];
        let m = Minutes::build(&t, DEFAULT_SLIPPAGE_FILTER);
        let rows = m.oriented("a", "b");
        assert_eq!(rows.len(), 2);
        let r = &rows[0];
        assert_eq!((r.ts, r.count, r.base_volume, r.counter_volume), (60_000, 3, 200, 90));
        assert_eq!(r.open, Rat { n: 1, d: 2 });
        assert_eq!(r.close, Rat { n: 1, d: 1 });
        assert_eq!(r.high, Rat { n: 1, d: 1 });
        assert_eq!(r.low, Rat { n: 1, d: 3 });
        let inv = m.oriented("b", "a");
        assert_eq!(inv[0].base_volume, 90);
        assert_eq!(inv[0].high, Rat { n: 3, d: 1 });
        assert_eq!(inv[0].low, Rat { n: 1, d: 1 });
        let j = rows[0].to_json();
        assert_eq!(j["timestamp"], "60000");
        assert_eq!(j["avg"], "0.4500000");
        assert_eq!(j["high_r"]["n"], "1");
    }

    #[test]
    fn tie_keeps_later_representation() {
        let t = vec![trade(0, 1, "a", "b", 1, 1, 1, 1), trade(1, 2, "a", "b", 2, 2, 2, 2)];
        let rows = Minutes::build(&t, 1000).oriented("a", "b");
        assert_eq!(rows[0].high, Rat { n: 2, d: 2 });
        assert_eq!(rows[0].low, Rat { n: 2, d: 2 });
    }

    #[test]
    fn buckets_limits_and_order() {
        let t: Vec<Trade> = (0..10).map(|i| trade(60 * i, i as i64 + 1, "a", "b", 10, 10 + i as i64, 1, 1)).collect();
        let rows = Minutes::build(&t, 1000).oriented("a", "b");
        let five = aggregate(&rows, &q(300_000, false, 200)).unwrap();
        assert_eq!(five.len(), 2);
        assert_eq!((five[0].ts, five[0].count), (0, 5));
        assert_eq!((five[1].ts, five[1].count), (300_000, 5));
        let desc = aggregate(&rows, &q(60_000, true, 3)).unwrap();
        assert_eq!(desc.iter().map(|r| r.ts).collect::<Vec<_>>(), vec![540_000, 480_000, 420_000]);
        let asc = aggregate(&rows, &q(60_000, false, 2)).unwrap();
        assert_eq!(asc.iter().map(|r| r.ts).collect::<Vec<_>>(), vec![0, 60_000]);
        let mut win = q(60_000, false, 200);
        win.start_time = Some(61_000);
        win.end_time = Some(240_000);
        let w = aggregate(&rows, &win).unwrap();
        assert_eq!(w.iter().map(|r| r.ts).collect::<Vec<_>>(), vec![120_000, 180_000]);
    }

    #[test]
    fn validation() {
        assert!(q(1000, false, 1).normalised().is_err());
        let mut x = q(3_600_000, false, 1);
        x.offset = 1_800_000;
        assert!(x.normalised().is_err());
        let mut y = q(60_000, false, 1);
        y.start_time = Some(120_000);
        y.end_time = Some(60_000);
        assert!(y.normalised().is_err());
    }

    #[test]
    fn slippage_filter_drops_pool_trades() {
        let mut t = trade(0, 1, "a", "b", 1, 1, 1, 1);
        t.rounding_slippage = Some(1000);
        let m = Minutes::build(&[t], 1000);
        assert_eq!(m.filtered, 1);
        assert!(m.oriented("a", "b").is_empty());
    }
}
