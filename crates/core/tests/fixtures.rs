// SPDX-License-Identifier: Apache-2.0
//! Offline tests over captured mainnet fixtures (see TESTING.md and the
//! `*.provenance.json` files beside each fixture set).

use flate2::read::GzDecoder;
use sdx_core::aggregate::Minutes;
use sdx_core::horizon;
use sdx_core::lake::{self, Lake};
use sdx_core::state::{ApplyStats, BookState};
use sdx_core::{bucket, ledger, trades};
use serde_json::Value;
use std::path::PathBuf;
use stellar_xdr::{LedgerEntry, Limits, ReadXdr};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Reads `.json` or zstd-compressed `.json.zst` (diff cases are stored compressed).
fn read_json(p: &std::path::Path) -> Value {
    let raw = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let raw = if p.extension().is_some_and(|e| e == "zst") { zstd::stream::decode_all(&raw[..]).unwrap() } else { raw };
    serde_json::from_slice(&raw).unwrap()
}

#[test]
fn level0_buckets_hash_verify_and_decode() {
    let dir = root().join("buckets");
    let prov = read_json(&dir.join("buckets.provenance.json"));
    let files = prov["files"].as_object().expect("files");
    assert!(!files.is_empty());
    for name in files.keys() {
        let hash = name.strip_prefix("bucket-").and_then(|s| s.strip_suffix(".xdr.gz")).unwrap();
        let f = std::fs::File::open(dir.join(name)).unwrap();
        let x = bucket::extract_from_reader(hash, GzDecoder::new(f), false).unwrap();
        assert!(x.stats.hash_verified, "{name} must hash to its name when read to EOF");
        assert!(x.stats.records_read > 0);
        // entry types must be non-decreasing inside a bucket (the early-exit premise)
        let by_type = x.stats.records_by_type;
        assert_eq!(by_type.iter().sum::<u64>() + 1, x.stats.records_read, "every record but METAENTRY has a type");
    }
}

#[test]
fn level0_bucket_types_are_sorted() {
    // Re-read the raw records and check the type sequence never decreases.
    let dir = root().join("buckets");
    let prov = read_json(&dir.join("buckets.provenance.json"));
    for name in prov["files"].as_object().unwrap().keys() {
        let mut r = GzDecoder::new(std::fs::File::open(dir.join(name)).unwrap());
        let mut raw = Vec::new();
        std::io::Read::read_to_end(&mut r, &mut raw).unwrap();
        let mut at = 0;
        let mut last = -1i32;
        while at + 4 <= raw.len() {
            let len = (u32::from_be_bytes(raw[at..at + 4].try_into().unwrap()) & 0x7fff_ffff) as usize;
            let rec = &raw[at + 4..at + 4 + len];
            let kind = i32::from_be_bytes(rec[0..4].try_into().unwrap());
            let t = match kind {
                0 | 2 => i32::from_be_bytes(rec[8..12].try_into().unwrap()),
                1 => i32::from_be_bytes(rec[4..8].try_into().unwrap()),
                _ => last,
            };
            assert!(t >= last, "{name}: entry type {t} after {last}");
            last = t;
            at += 4 + len;
        }
    }
}

fn mirror() -> Lake {
    Lake::new(format!("file://{}", root().join("lake/mirror").display()), None)
}

fn lake_range() -> (u32, u32) {
    let p = read_json(&root().join("lake/lake.provenance.json"));
    (p["ledgers"][0].as_u64().unwrap() as u32, p["ledgers"][1].as_u64().unwrap() as u32)
}

/// Horizon trade record -> (paging token, sold asset, bought asset, amount sold, amount bought, price n, price d)
fn horizon_trade(r: &Value) -> (String, String, String, i64, i64, i64, i64) {
    let asset = |p: &str| {
        let t = r[format!("{p}_asset_type")].as_str().unwrap();
        if t == "native" {
            "native".to_string()
        } else {
            format!("{t}/{}/{}", r[format!("{p}_asset_code")].as_str().unwrap(), r[format!("{p}_asset_issuer")].as_str().unwrap())
        }
    };
    let amt = |k: &str| sdx_core::amount::parse(r[k].as_str().unwrap()).unwrap();
    let n: i64 = r["price"]["n"].as_str().map(|s| s.parse().unwrap()).or(r["price"]["n"].as_i64()).unwrap();
    let d: i64 = r["price"]["d"].as_str().map(|s| s.parse().unwrap()).or(r["price"]["d"].as_i64()).unwrap();
    let tok = r["paging_token"].as_str().unwrap().to_string();
    if r["base_is_seller"].as_bool().unwrap() {
        (tok, asset("base"), asset("counter"), amt("base_amount"), amt("counter_amount"), n, d)
    } else {
        (tok, asset("counter"), asset("base"), amt("counter_amount"), amt("base_amount"), d, n)
    }
}

#[test]
fn derived_trades_equal_horizon_trades() {
    let (first, last) = lake_range();
    let lake = mirror();
    let mut total = 0;
    for seq in first..=last {
        let bytes = lake.fetch(seq).unwrap().expect("fixture object");
        let metas = lake::decode(&bytes).unwrap();
        assert_eq!(metas.len(), 1);
        let v = ledger::view(&metas[0]);
        assert_eq!(v.sequence(), seq);
        let (ours, errs) = trades::from_view(&v);
        assert!(errs.is_empty(), "{errs:?}");
        let h = read_json(&root().join(format!("lake/horizon-trades-{seq}.json")));
        let mut want: Vec<_> = h["records"].as_array().unwrap().iter().map(horizon_trade).collect();
        let mut got: Vec<_> = ours
            .iter()
            .map(|t| {
                (format!("{}-{}", t.op_id, t.order), t.sold.clone(), t.bought.clone(), t.amount_sold, t.amount_bought, t.price_n, t.price_d)
            })
            .collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "ledger {seq}");
        total += got.len();
    }
    assert!(total > 0, "the fixture range should contain trades");
}

#[test]
fn replaying_fixture_ledgers_from_their_own_preimages_is_consistent() {
    // Seed a state with every offer/pool STATE pre-image seen in the first
    // ledger, then apply all fixture ledgers strictly: any mismatch means
    // our apply order or change handling is wrong.
    let (first, last) = lake_range();
    let lake = mirror();
    let mut state = BookState::new();
    let mut st = ApplyStats::default();
    let mut seeded = std::collections::HashSet::new();
    let mut created = std::collections::HashSet::new();
    let mut metas = Vec::new();
    for seq in first..=last {
        metas.extend(lake::decode(&lake.fetch(seq).unwrap().unwrap()).unwrap());
    }
    // Seed: first time a key is seen across the range, if it is a STATE, it is the pre-state.
    for m in &metas {
        let v = ledger::view(m);
        let mut all = Vec::new();
        for t in &v.txs {
            all.push(t.fee_processing);
        }
        for t in &v.txs {
            let (b, o, a) = ledger::tx_changes(t.meta);
            all.extend(b);
            all.extend(o);
            all.extend(a);
        }
        for c in all.into_iter().flat_map(|c| c.iter()) {
            use stellar_xdr::LedgerEntryChange as C;
            let (key, is_state, e) = match c {
                C::State(e) => (e.to_key(), true, Some(e)),
                C::Created(e) | C::Updated(e) | C::Restored(e) => (e.to_key(), false, Some(e)),
                C::Removed(k) => (k.clone(), false, None),
            };
            if !matches!(key, stellar_xdr::LedgerKey::Offer(_) | stellar_xdr::LedgerKey::LiquidityPool(_)) {
                continue;
            }
            if seeded.contains(&key) || created.contains(&key) {
                continue;
            }
            if is_state {
                state.upsert(e.unwrap(), &mut st).unwrap();
                seeded.insert(key);
            } else {
                created.insert(key);
            }
        }
    }
    assert!(state.offer_count() > 0);
    state.ledger = first - 1;
    let mut checked = 0;
    for m in &metas {
        let out = ledger::apply(&mut state, m, true).unwrap();
        checked += out.stats.state_checked;
    }
    assert!(checked > 0);
    assert_eq!(state.ledger, last);
}

fn diff_runs() -> Vec<PathBuf> {
    let d = root().join("diff");
    let mut v: Vec<PathBuf> =
        std::fs::read_dir(&d).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn substate(case: &Value) -> BookState {
    let mut s = BookState::new();
    let mut st = ApplyStats::default();
    for side in ["asks", "bids"] {
        for hx in case["substate"][side].as_array().unwrap() {
            let e = LedgerEntry::from_xdr(hex::decode(hx.as_str().unwrap()).unwrap(), Limits::none()).unwrap();
            s.upsert(&e, &mut st).unwrap();
        }
    }
    s
}

#[test]
fn diff_fixtures_reproduce_offline() {
    let runs = diff_runs();
    assert!(!runs.is_empty(), "expected at least one fixtures/diff/<run>");
    let mut books = 0;
    let mut aggs = 0;
    for run in runs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(run.join("cases"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".json") || p.to_string_lossy().ends_with(".json.zst"))
            .collect();
        files.sort();
        for f in files {
            let c = read_json(&f);
            let kind = c["case"]["kind"].as_str().unwrap();
            if c.get("compare").is_none() {
                continue;
            }
            let query = c["request"]["query"].as_str().unwrap();
            let p = horizon::parse_query(query);
            match kind {
                "order_book" => {
                    let s = substate(&c);
                    let ours = horizon::order_book(&s, &p).unwrap();
                    assert_eq!(ours, c["ours"], "{}: recomputed order book differs from the recorded answer", f.display());
                    if c["compare"]["equal"] == true {
                        assert_eq!(ours, c["horizon"]["body"], "{}", f.display());
                    }
                    books += 1;
                }
                "trade_aggregations" => {
                    let trades: Vec<trades::Trade> = serde_json::from_value(c["trades"].clone()).unwrap();
                    let m = Minutes::build(&trades, sdx_core::aggregate::DEFAULT_SLIPPAGE_FILTER);
                    let ours = horizon::trade_aggregations(&m, &p, "").unwrap();
                    assert_eq!(ours["_embedded"], c["ours"]["_embedded"], "{}", f.display());
                    if c["compare"]["equal"] == true {
                        assert_eq!(ours["_embedded"], c["horizon"]["body"]["_embedded"], "{}", f.display());
                    }
                    aggs += 1;
                }
                _ => {}
            }
        }
    }
    assert!(books > 0);
    assert!(aggs > 0);
}
