// SPDX-License-Identifier: Apache-2.0
//! Differential check against a live Horizon at matching ledgers (SPEC 6).
//!
//! For each live case (order book, strict-send, strict-receive) we query
//! Horizon first and record its `Latest-Ledger` header L, then replay our
//! state ledger by ledger and answer each case at its L (and at L-1), so the
//! comparison is ledger-exact. Cases answered by Horizon for a ledger our
//! state has already passed are recorded as skipped.
//! Trade aggregations are compared over complete minutes inside the replayed
//! window. Every case is written with both bodies so a mismatch is a fixture.

use crate::app::App;
use sdx_core::asset;
use sdx_core::book;
use sdx_core::error::{Code, Error, Result};
use sdx_core::horizon::{self, Params};
use sdx_core::lake::Lake;
use sdx_core::trades::Trade;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Case {
    OrderBook { selling: String, buying: String, limit: Option<u32> },
    StrictSend { source: String, amount: String, destinations: Vec<String> },
    StrictReceive { destination: String, amount: String, sources: Vec<String> },
    TradeAggregations { base: String, counter: String, resolution: i64 },
}

fn asset_params(prefix: &str, canonical: &str, out: &mut Vec<(String, String)>) {
    match asset::split(canonical) {
        None => out.push((format!("{prefix}asset_type"), "native".into())),
        Some((t, c, i)) => {
            out.push((format!("{prefix}asset_type"), t.into()));
            out.push((format!("{prefix}asset_code"), c.into()));
            out.push((format!("{prefix}asset_issuer"), i.into()));
        }
    }
}

fn qs(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", v.replace(':', "%3A").replace(',', "%2C"))).collect::<Vec<_>>().join("&")
}

impl Case {
    /// (path, query) for Horizon and for our router. Assets are canonical or short form.
    pub fn request(&self, window: Option<(i64, i64)>) -> Result<(String, String)> {
        let mut p: Vec<(String, String)> = Vec::new();
        let path = match self {
            Case::OrderBook { selling, buying, limit } => {
                asset_params("selling_", &asset::parse_short(selling)?, &mut p);
                asset_params("buying_", &asset::parse_short(buying)?, &mut p);
                p.push(("limit".into(), limit.unwrap_or(20).to_string()));
                "/order_book"
            }
            Case::StrictSend { source, amount, destinations } => {
                asset_params("source_", &asset::parse_short(source)?, &mut p);
                p.push(("source_amount".into(), amount.clone()));
                p.push(("destination_assets".into(), destinations.join(",")));
                "/paths/strict-send"
            }
            Case::StrictReceive { destination, amount, sources } => {
                asset_params("destination_", &asset::parse_short(destination)?, &mut p);
                p.push(("destination_amount".into(), amount.clone()));
                p.push(("source_assets".into(), sources.join(",")));
                "/paths/strict-receive"
            }
            Case::TradeAggregations { base, counter, resolution } => {
                asset_params("base_", &asset::parse_short(base)?, &mut p);
                asset_params("counter_", &asset::parse_short(counter)?, &mut p);
                p.push(("resolution".into(), resolution.to_string()));
                if let Some((s, e)) = window {
                    p.push(("start_time".into(), s.to_string()));
                    p.push(("end_time".into(), e.to_string()));
                }
                p.push(("limit".into(), "200".into()));
                p.push(("order".into(), "asc".into()));
                "/trade_aggregations"
            }
        };
        Ok((path.to_string(), qs(&p)))
    }

    fn kind(&self) -> &'static str {
        match self {
            Case::OrderBook { .. } => "order_book",
            Case::StrictSend { .. } => "strict_send",
            Case::StrictReceive { .. } => "strict_receive",
            Case::TradeAggregations { .. } => "trade_aggregations",
        }
    }
}

/// Our answer for a case at the app's current ledger.
pub fn ours(app: &App, case: &Case, query: &str) -> Value {
    let p: Params = horizon::parse_query(query);
    let r = match case {
        Case::OrderBook { .. } => horizon::order_book(&app.book, &p),
        Case::StrictSend { .. } => horizon::strict_send(&app.book, &p, app.max_path_length, app.include_pools),
        Case::StrictReceive { .. } => horizon::strict_receive(&app.book, &p, app.max_path_length, app.include_pools),
        Case::TradeAggregations { .. } => horizon::trade_aggregations(&app.minutes, &p, ""),
    };
    r.unwrap_or_else(|e| horizon::problem(&e))
}

pub struct HorizonClient {
    pub base: String,
    agent: ureq::Agent,
    last: Option<Instant>,
    pub min_gap: Duration,
    pub requests: u64,
}

impl HorizonClient {
    pub fn new(base: &str) -> HorizonClient {
        HorizonClient {
            base: base.trim_end_matches('/').to_string(),
            agent: sdx_core::lake::agent(),
            last: None,
            min_gap: Duration::from_millis(600),
            requests: 0,
        }
    }

    /// GET; returns (status, Latest-Ledger header, body JSON).
    pub fn get(&mut self, path: &str, query: &str) -> Result<(u16, Option<u32>, Value)> {
        if let Some(t) = self.last {
            let e = t.elapsed();
            if e < self.min_gap {
                std::thread::sleep(self.min_gap - e);
            }
        }
        self.last = Some(Instant::now());
        self.requests += 1;
        let url = format!("{}{}?{}", self.base, path, query);
        let resp = match self.agent.get(&url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(Error::new(Code::Fetch, format!("{url}: {e}"))),
        };
        let status = resp.status();
        let ledger = resp.header("Latest-Ledger").and_then(|v| v.parse().ok());
        let mut body = String::new();
        resp.into_reader().read_to_string(&mut body).map_err(|e| Error::new(Code::Fetch, format!("{url}: {e}")))?;
        let v = serde_json::from_str(&body).unwrap_or(Value::String(body));
        Ok((status, ledger, v))
    }
}

/// Field-level comparison summary.
pub fn compare(case: &Case, horizon: &Value, ours: &Value) -> Value {
    match case {
        Case::OrderBook { .. } => {
            let side = |k: &str| {
                let h = horizon[k].as_array().cloned().unwrap_or_default();
                let o = ours[k].as_array().cloned().unwrap_or_default();
                let first_diff =
                    h.iter().zip(o.iter()).position(|(a, b)| a != b).or(if h.len() != o.len() { Some(h.len().min(o.len())) } else { None });
                json!({"horizon_levels": h.len(), "our_levels": o.len(), "equal": h == o, "first_diff_index": first_diff})
            };
            json!({"bids": side("bids"), "asks": side("asks"), "equal": horizon["bids"] == ours["bids"] && horizon["asks"] == ours["asks"]})
        }
        Case::StrictSend { .. } | Case::StrictReceive { .. } => {
            let h = horizon["_embedded"]["records"].as_array().cloned().unwrap_or_default();
            let o = ours["_embedded"]["records"].as_array().cloned().unwrap_or_default();
            let key = |r: &Value| r.to_string();
            let mut hs: Vec<String> = h.iter().map(key).collect();
            let mut os: Vec<String> = o.iter().map(key).collect();
            hs.sort();
            os.sort();
            let amount_key = if matches!(case, Case::StrictSend { .. }) { "destination_amount" } else { "source_amount" };
            json!({
                "horizon_records": h.len(),
                "our_records": o.len(),
                "equal": h == o,
                "equal_ignoring_order": hs == os,
                "best_equal": h.first().map(|r| &r[amount_key]) == o.first().map(|r| &r[amount_key]),
                "horizon_best": h.first().map(|r| r[amount_key].clone()),
                "our_best": o.first().map(|r| r[amount_key].clone()),
            })
        }
        Case::TradeAggregations { .. } => {
            let h = horizon["_embedded"]["records"].as_array().cloned().unwrap_or_default();
            let o = ours["_embedded"]["records"].as_array().cloned().unwrap_or_default();
            let mut field_mismatch: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
            let ho: std::collections::BTreeMap<String, &Value> = o.iter().map(|r| (r["timestamp"].to_string(), r)).collect();
            let mut missing = 0;
            for r in &h {
                match ho.get(&r["timestamp"].to_string()) {
                    None => missing += 1,
                    Some(x) => {
                        for (k, v) in r.as_object().into_iter().flatten() {
                            if &x[k] != v {
                                *field_mismatch.entry(k.clone()).or_default() += 1;
                            }
                        }
                    }
                }
            }
            json!({
                "horizon_buckets": h.len(),
                "our_buckets": o.len(),
                "equal": h == o,
                "buckets_missing_in_ours": missing,
                "field_mismatches": field_mismatch,
            })
        }
    }
}

/// Offers of both books of a pair at the current ledger (hex XDR), so an
/// order-book case can be re-checked offline.
fn pair_substate(app: &App, selling: &str, buying: &str) -> Value {
    use stellar_xdr::{Limits, WriteXdr};
    let enc = |s: &str, b: &str| -> Vec<String> {
        match (app.book.assets.get(s), app.book.assets.get(b)) {
            (Some(x), Some(y)) => app.book.book(x, y).filter_map(|o| o.entry.to_xdr(Limits::none()).ok()).map(hex::encode).collect(),
            _ => vec![],
        }
    };
    json!({"asks": enc(selling, buying), "bids": enc(buying, selling)})
}

fn pair_trades<'a>(trades: &'a [Trade], a: &str, b: &str, from_ms: i64, to_ms: i64) -> Vec<&'a Trade> {
    trades
        .iter()
        .filter(|t| {
            ((t.sold == a && t.bought == b) || (t.sold == b && t.bought == a))
                && (t.close_time as i64 * 1000) >= from_ms
                && (t.close_time as i64 * 1000) < to_ms
        })
        .collect()
}

#[derive(Debug, Default, Serialize)]
pub struct KindSummary {
    pub cases: u64,
    pub compared_at_matching_ledger: u64,
    pub equal: u64,
    pub equal_ignoring_order: u64,
    pub best_equal: u64,
    pub mismatched: u64,
    pub matched_previous_ledger: u64,
    pub skipped: u64,
}

pub struct DiffOptions<'a> {
    pub out: &'a Path,
    pub lake: &'a Lake,
    pub workers: usize,
    pub trades_all: Vec<Trade>,
}

/// Run all cases; writes `<out>/cases/NNN-<kind>.json` and `<out>/summary.json`.
///
/// Record-then-replay: all live cases are first queried from Horizon (each
/// answer carries its `Latest-Ledger` H); then our state is replayed ledger by
/// ledger and every case is answered exactly at its H (and at H-1 for the
/// off-by-one diagnosis). This keeps comparisons ledger-exact even when our
/// replay is slower than the network.
pub fn run(app: &mut App, client: &mut HorizonClient, cases: &[Case], opts: DiffOptions<'_>) -> Result<Value> {
    let t0 = Instant::now();
    let cases_dir = opts.out.join("cases");
    std::fs::create_dir_all(&cases_dir).map_err(|e| Error::io(&cases_dir, e))?;
    let mut summary: std::collections::BTreeMap<&'static str, KindSummary> = std::collections::BTreeMap::new();
    let progress = |a: &App| eprint!("\rsdx diff: replayed to {}   ", a.book.ledger);
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

    // 1. Query Horizon for every live case.
    struct Pending {
        i: usize,
        ledger: u32,
        rec: Value,
        hbody: Value,
        query: String,
    }
    let mut pending: Vec<Pending> = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        if matches!(case, Case::TradeAggregations { .. }) {
            continue;
        }
        let s = summary.entry(case.kind()).or_default();
        s.cases += 1;
        let (path, query) = case.request(None)?;
        let (status, hl, hbody) = client.get(&path, &query)?;
        let mut rec = json!({"index": i, "case": case, "captured_at_unix": now(), "request": {"path": path, "query": query}});
        rec["horizon"] = json!({"status": status, "body": hbody.clone()});
        match hl {
            Some(h) if h > app.book.ledger => {
                rec["ledger"] = json!(h);
                pending.push(Pending { i, ledger: h, rec, hbody, query });
            }
            other => {
                s.skipped += 1;
                rec["result"] = json!(if other.is_none() { "no_latest_ledger_header" } else { "horizon_not_ahead_of_state" });
                rec["horizon_latest_ledger"] = json!(other);
                write_case(&cases_dir, i, case, &rec)?;
            }
        }
    }
    eprintln!("sdx diff: {} live cases recorded from Horizon; replaying from {}", pending.len(), app.book.ledger);

    // 2. Replay and answer each case at its ledger.
    pending.sort_by_key(|p| (p.ledger, p.i));
    let mut k = 0;
    while k < pending.len() {
        let h = pending[k].ledger;
        let group_end = pending[k..].iter().position(|p| p.ledger != h).map_or(pending.len(), |x| k + x);
        if h > app.book.ledger + 1 {
            app.replay_to(opts.lake, h - 1, opts.workers, false, 300, progress)?;
        }
        let prev: Vec<Value> = pending[k..group_end].iter().map(|p| ours(app, &cases[p.i], &p.query)).collect();
        app.replay_to(opts.lake, h, opts.workers, false, 300, progress)?;
        for (j, p) in pending[k..group_end].iter_mut().enumerate() {
            let case = &cases[p.i];
            let s = summary.entry(case.kind()).or_default();
            let ob = ours(app, case, &p.query);
            let cmp = compare(case, &p.hbody, &ob);
            s.compared_at_matching_ledger += 1;
            let equal = cmp["equal"] == true;
            if equal {
                s.equal += 1;
            } else {
                s.mismatched += 1;
                if compare(case, &p.hbody, &prev[j])["equal"] == true {
                    s.matched_previous_ledger += 1;
                    p.rec["matched_previous_ledger"] = json!(true);
                }
            }
            if cmp.get("equal_ignoring_order") == Some(&json!(true)) {
                s.equal_ignoring_order += 1;
            }
            if cmp.get("best_equal") == Some(&json!(true)) {
                s.best_equal += 1;
            }
            p.rec["our_ledger"] = json!(app.book.ledger);
            p.rec["ours"] = ob;
            p.rec["compare"] = cmp;
            if let Case::OrderBook { selling, buying, .. } = case {
                p.rec["substate"] = pair_substate(app, &asset::parse_short(selling)?, &asset::parse_short(buying)?);
            }
            write_case(&cases_dir, p.i, case, &p.rec)?;
        }
        k = group_end;
    }
    eprintln!();

    // 3. Trade aggregations over complete minutes of the replayed window.
    let trades_all = if opts.trades_all.last().map(|t| t.ledger).unwrap_or(0) < app.book.ledger {
        sdx_core::snapshot::load_trades(&app.dir, app.book.ledger)?
    } else {
        opts.trades_all
    };
    for (i, case) in cases.iter().enumerate() {
        let Case::TradeAggregations { base, counter, .. } = case else { continue };
        let s = summary.entry(case.kind()).or_default();
        s.cases += 1;
        let mut rec = json!({"index": i, "case": case, "captured_at_unix": now()});
        let first_close = trades_all.iter().map(|t| t.close_time).min().unwrap_or(app.book.close_time) as i64 * 1000;
        let start = (first_close / 60_000 + 1) * 60_000;
        let end = (app.book.close_time as i64 * 1000 - 120_000) / 60_000 * 60_000;
        if end <= start {
            s.skipped += 1;
            rec["result"] = json!("skipped_window_too_short");
            write_case(&cases_dir, i, case, &rec)?;
            continue;
        }
        let (path, query) = case.request(Some((start, end)))?;
        let (status, _, hbody) = client.get(&path, &query)?;
        let ob = ours(app, case, &query);
        let cmp = compare(case, &hbody, &ob);
        s.compared_at_matching_ledger += 1;
        if cmp["equal"] == true {
            s.equal += 1;
        } else {
            s.mismatched += 1;
        }
        let (ba, ca) = (asset::parse_short(base)?, asset::parse_short(counter)?);
        rec["request"] = json!({"path": path, "query": query, "window_ms": [start, end]});
        rec["our_ledger"] = json!(app.book.ledger);
        rec["horizon"] = json!({"status": status, "body": hbody});
        rec["ours"] = ob;
        rec["compare"] = cmp;
        rec["trades"] = serde_json::to_value(pair_trades(&trades_all, &ba, &ca, start, end)).unwrap_or_default();
        write_case(&cases_dir, i, case, &rec)?;
    }
    let out = json!({
        "horizon": client.base,
        "horizon_requests": client.requests,
        "final_ledger": app.book.ledger,
        "seconds": t0.elapsed().as_secs_f64(),
        "kinds": summary,
        "replay_totals": app.totals,
        "trade_errors": app.trade_errors,
    });
    let p = opts.out.join("summary.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&out).unwrap_or_default()).map_err(|e| Error::io(&p, e))?;
    Ok(out)
}

fn write_case(dir: &Path, i: usize, case: &Case, rec: &Value) -> Result<()> {
    let p = dir.join(format!("{i:03}-{}.json", case.kind()));
    std::fs::write(&p, serde_json::to_vec_pretty(rec).unwrap_or_default()).map_err(|e| Error::io(&p, e))
}

/// Cases sampled from the current state: the `n` pairs with the most offers
/// (both sides counted), amounts at three sizes, plus trade-aggregation cases
/// for the `n` pairs with the most trade minutes.
pub fn auto_cases(app: &App, n: usize) -> Vec<Case> {
    let mut pairs: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
    for o in app.book.offers_sorted() {
        let (a, b) = (app.book.assets.name(o.selling).to_string(), app.book.assets.name(o.buying).to_string());
        let k = if a < b { (a, b) } else { (b, a) };
        *pairs.entry(k).or_default() += 1;
    }
    let mut v: Vec<((String, String), usize)> = pairs.into_iter().collect();
    v.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let mut cases = Vec::new();
    for ((a, b), _) in v.iter().take(n) {
        // native first as base when present, as wallets usually ask
        let (base, counter) = if b == "native" { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
        let (bs, cs) = (asset::short(&base), asset::short(&counter));
        cases.push(Case::OrderBook { selling: bs.clone(), buying: cs.clone(), limit: Some(20) });
        // amounts sized from the best ask so they are meaningful for any price scale
        let s = book::summary(&app.book, &base, &counter, 1);
        let unit = s.asks.first().map(|l| l.amount.clamp(1, 1_000_000_000_000)).unwrap_or(10_000_000);
        for frac in [1000i128, 10, 1] {
            let amt = sdx_core::amount::format((unit / frac).max(1));
            cases.push(Case::StrictSend { source: bs.clone(), amount: amt.clone(), destinations: vec![cs.clone()] });
            cases.push(Case::StrictReceive { destination: bs.clone(), amount: amt, sources: vec![cs.clone()] });
        }
    }
    let mut tpairs: Vec<(String, String, usize)> = app.minutes.pairs().map(|(a, b, n)| (a.to_string(), b.to_string(), n)).collect();
    tpairs.sort_by(|x, y| y.2.cmp(&x.2).then(x.0.cmp(&y.0)));
    for (a, b, _) in tpairs.into_iter().take(n) {
        for res in [60_000, 300_000] {
            cases.push(Case::TradeAggregations { base: asset::short(&a), counter: asset::short(&b), resolution: res });
        }
    }
    cases
}
