// SPDX-License-Identifier: Apache-2.0
//! `sdx`: bootstrap, replay, query, serve and diff the Horizon-free SDEX
//! order book. Read-only on the network; never signs or sends anything.

mod app;
mod diff;
mod http;

use app::App;
use clap::{Parser, Subcommand};
use sdx_core::bucket::{self, SDF_ARCHIVE};
use sdx_core::horizon;
use sdx_core::lake::{Lake, LAKE_URL};
use sdx_core::paths;
use sdx_core::snapshot;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};

#[derive(Parser)]
#[command(name = "sdx", version, about = "Horizon-free SDEX order book, path finding and trade aggregation from ledger data")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args, Clone)]
struct QueryOpts {
    /// State directory written by `bootstrap` / `replay`.
    #[arg(long, default_value = "data/state")]
    state: PathBuf,
    /// Maximum path length (Horizon default 3, max 5).
    #[arg(long, default_value_t = paths::DEFAULT_MAX_PATH_LENGTH)]
    max_path_length: usize,
    /// Leave classic liquidity pools out of path finding.
    #[arg(long)]
    no_pools: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the offer + pool state at a checkpoint from history-archive buckets.
    Bootstrap {
        #[arg(long, default_value = "data/state")]
        out: PathBuf,
        #[arg(long, default_value = SDF_ARCHIVE)]
        archive: String,
        /// Checkpoint ledger ((n+1) % 64 == 0); latest when omitted.
        #[arg(long)]
        checkpoint: Option<u32>,
        /// Cache of per-bucket offer/pool extracts (content-addressed, reusable).
        #[arg(long, default_value = "data/bucket-cache")]
        cache: PathBuf,
        /// Concurrent bucket downloads.
        #[arg(long, default_value_t = 3)]
        workers: usize,
    },
    /// Apply ledgers from the Galexie lake to a state directory.
    Replay {
        #[arg(long, default_value = "data/state")]
        state: PathBuf,
        /// Target ledger, or "head" for the newest ledger Horizon-free sources know (lake listing).
        #[arg(long)]
        to: String,
        #[arg(long, default_value = LAKE_URL)]
        lake: String,
        /// Cache directory for raw lake objects (omit to keep nothing).
        #[arg(long)]
        lake_cache: Option<PathBuf>,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        /// Fail on the first STATE pre-image mismatch instead of counting it.
        #[arg(long)]
        strict: bool,
        /// Save the state every N ledgers (0 = only at the end).
        #[arg(long, default_value_t = 256)]
        save_every: u32,
    },
    /// Print `/order_book` JSON.
    OrderBook {
        #[command(flatten)]
        q: QueryOpts,
        /// `native` or CODE:ISSUER
        #[arg(long)]
        selling: String,
        #[arg(long)]
        buying: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Print `/paths/strict-send` JSON.
    StrictSend {
        #[command(flatten)]
        q: QueryOpts,
        #[arg(long)]
        source: String,
        #[arg(long)]
        amount: String,
        /// Comma-separated destination assets.
        #[arg(long)]
        destinations: String,
    },
    /// Print `/paths/strict-receive` JSON.
    StrictReceive {
        #[command(flatten)]
        q: QueryOpts,
        #[arg(long)]
        destination: String,
        #[arg(long)]
        amount: String,
        /// Comma-separated source assets.
        #[arg(long)]
        sources: String,
    },
    /// Print `/trade_aggregations` JSON over the replayed trades.
    TradeAggregations {
        #[command(flatten)]
        q: QueryOpts,
        #[arg(long)]
        base: String,
        #[arg(long)]
        counter: String,
        #[arg(long, default_value_t = 60_000)]
        resolution: i64,
        #[arg(long)]
        start_time: Option<i64>,
        #[arg(long)]
        end_time: Option<i64>,
        #[arg(long, default_value_t = 0)]
        offset: i64,
        #[arg(long, default_value = "asc")]
        order: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Serve the Horizon-compatible endpoints on a local address.
    Serve {
        #[command(flatten)]
        q: QueryOpts,
        #[arg(long, default_value = "127.0.0.1:8000")]
        addr: String,
        /// Keep applying new ledgers from the lake while serving.
        #[arg(long)]
        follow: bool,
        #[arg(long, default_value = LAKE_URL)]
        lake: String,
    },
    /// Differential check against a live Horizon at matching ledgers.
    Diff {
        #[command(flatten)]
        q: QueryOpts,
        #[arg(long, default_value = "https://horizon.stellar.org")]
        horizon: String,
        /// JSON array of cases; omitted = sampled from the state (`--auto`).
        #[arg(long)]
        cases: Option<PathBuf>,
        /// Number of pairs to sample when no cases file is given.
        #[arg(long, default_value_t = 8)]
        auto: usize,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = LAKE_URL)]
        lake: String,
        #[arg(long, default_value_t = 4)]
        workers: usize,
    },
    /// Print the state manifest and counts.
    Status {
        #[command(flatten)]
        q: QueryOpts,
    },
}

fn open(q: &QueryOpts) -> sdx_core::Result<App> {
    App::open(&q.state, q.max_path_length, !q.no_pools)
}

fn print(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn asset_q(prefix: &str, short: &str, out: &mut horizon::Params) -> sdx_core::Result<()> {
    let c = sdx_core::asset::parse_short(short)?;
    match sdx_core::asset::split(&c) {
        None => {
            out.insert(format!("{prefix}asset_type"), "native".into());
        }
        Some((t, code, issuer)) => {
            out.insert(format!("{prefix}asset_type"), t.into());
            out.insert(format!("{prefix}asset_code"), code.into());
            out.insert(format!("{prefix}asset_issuer"), issuer.into());
        }
    }
    Ok(())
}

fn lake_head(lake: &Lake, from: u32) -> sdx_core::Result<u32> {
    // Probe forward by doubling then bisect: HEAD-like GETs of small objects.
    let exists = |s: u32| lake.fetch(s).map(|o| o.is_some());
    let mut lo = from;
    if !exists(lo)? {
        return Ok(from.saturating_sub(1));
    }
    let mut step = 16u32;
    let mut hi = lo + step;
    while exists(hi)? {
        lo = hi;
        step *= 2;
        hi = lo + step;
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if exists(mid)? {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(lo)
}

fn run(cli: Cli) -> sdx_core::Result<()> {
    match cli.cmd {
        Cmd::Bootstrap { out, archive, checkpoint, cache, workers } => {
            let progress = |s: &bucket::ExtractStats| {
                eprintln!(
                    "sdx bootstrap: bucket {} records={} offers+{}/-{} pools+{}/-{} early_exit={} verified={} {:.1}s",
                    &s.hash[..12],
                    s.records_read,
                    s.offers_live,
                    s.offers_dead,
                    s.pools_live,
                    s.pools_dead,
                    s.early_exit,
                    s.hash_verified,
                    s.seconds
                )
            };
            let (mut state, has, report) = bucket::bootstrap(&archive, checkpoint, Some(&cache), workers, &progress)?;
            // Tie the HAS to the ledger chain: the checkpoint ledger header (from the lake)
            // commits to the bucket-list hash.
            let header_check = match Lake::public(None).fetch(has.current_ledger)? {
                Some(bytes) => {
                    let metas = sdx_core::lake::decode(&bytes)?;
                    let v = sdx_core::ledger::view(&metas[0]);
                    let h = &v.header.header;
                    let want = bucket::bucket_list_hash(&has, h.ledger_version)?;
                    state.close_time = v.close_time();
                    json!({"ledger": h.ledger_seq, "protocol": h.ledger_version, "header_bucket_list_hash": hex_of(&h.bucket_list_hash.0),
                           "computed_from_has": hex_of(&want), "match": h.bucket_list_hash.0 == want})
                }
                None => json!({"match": null, "reason": "checkpoint ledger not in lake"}),
            };
            eprintln!("sdx bootstrap: HAS vs ledger header: {header_check}");
            if header_check["match"] == json!(false) {
                return Err(sdx_core::Error::new(
                    sdx_core::error::Code::Consistency,
                    format!("HAS does not match ledger header: {header_check}"),
                ));
            }
            let m = snapshot::save(
                &out,
                &state,
                has.current_ledger,
                json!({"archive": archive, "has": has, "bootstrap_seconds": report.seconds, "has_header_check": header_check}),
            )?;
            let rp = out.join("bootstrap-report.json");
            std::fs::write(&rp, serde_json::to_vec_pretty(&report).unwrap_or_default()).map_err(|e| sdx_core::Error::io(&rp, e))?;
            print(&json!({"manifest": m, "seconds": report.seconds, "peak_rss_kib": report.peak_rss_kib}));
        }
        Cmd::Replay { state, to, lake, lake_cache, workers, strict, save_every } => {
            let mut app = App::open(&state, paths::DEFAULT_MAX_PATH_LENGTH, true)?;
            let lake = Lake::new(lake, lake_cache);
            let target = if to == "head" {
                lake_head(&lake, app.book.ledger + 1)?
            } else {
                to.parse().map_err(|_| sdx_core::Error::bad("--to must be a ledger or head"))?
            };
            let t0 = std::time::Instant::now();
            let start = app.book.ledger;
            let mut last_save = start;
            while app.book.ledger < target {
                let step_target = if save_every == 0 { target } else { target.min(app.book.ledger + save_every) };
                app.replay_to(&lake, step_target, workers, strict, 120, |a| {
                    eprint!(
                        "\rsdx replay: ledger {} offers {} pools {} mismatches {}   ",
                        a.book.ledger,
                        a.book.offer_count(),
                        a.book.pool_count(),
                        a.totals.state_mismatch
                    )
                })?;
                if app.book.ledger != last_save {
                    app.save(json!({"replayed_from": start}))?;
                    last_save = app.book.ledger;
                }
            }
            eprintln!();
            print(&json!({
                "from": start, "to": app.book.ledger, "seconds": t0.elapsed().as_secs_f64(),
                "totals": app.totals, "trade_errors": app.trade_errors, "peak_rss_kib": bucket::peak_rss_kib(),
            }));
        }
        Cmd::OrderBook { q, selling, buying, limit } => {
            let app = open(&q)?;
            let mut p = horizon::Params::new();
            asset_q("selling_", &selling, &mut p)?;
            asset_q("buying_", &buying, &mut p)?;
            p.insert("limit".into(), limit.to_string());
            print(&horizon::order_book(&app.book, &p)?);
        }
        Cmd::StrictSend { q, source, amount, destinations } => {
            let app = open(&q)?;
            let mut p = horizon::Params::new();
            asset_q("source_", &source, &mut p)?;
            p.insert("source_amount".into(), amount);
            p.insert("destination_assets".into(), destinations);
            print(&horizon::strict_send(&app.book, &p, app.max_path_length, app.include_pools)?);
        }
        Cmd::StrictReceive { q, destination, amount, sources } => {
            let app = open(&q)?;
            let mut p = horizon::Params::new();
            asset_q("destination_", &destination, &mut p)?;
            p.insert("destination_amount".into(), amount);
            p.insert("source_assets".into(), sources);
            print(&horizon::strict_receive(&app.book, &p, app.max_path_length, app.include_pools)?);
        }
        Cmd::TradeAggregations { q, base, counter, resolution, start_time, end_time, offset, order, limit } => {
            let app = open(&q)?;
            let mut p = horizon::Params::new();
            asset_q("base_", &base, &mut p)?;
            asset_q("counter_", &counter, &mut p)?;
            p.insert("resolution".into(), resolution.to_string());
            p.insert("offset".into(), offset.to_string());
            if let Some(s) = start_time {
                p.insert("start_time".into(), s.to_string());
            }
            if let Some(e) = end_time {
                p.insert("end_time".into(), e.to_string());
            }
            p.insert("order".into(), order);
            p.insert("limit".into(), limit.to_string());
            print(&horizon::trade_aggregations(&app.minutes, &p, "")?);
        }
        Cmd::Serve { q, addr, follow, lake } => {
            let app = Arc::new(RwLock::new(open(&q)?));
            if follow {
                let app2 = Arc::clone(&app);
                let lake = Lake::new(lake, None);
                std::thread::spawn(move || loop {
                    let next = app2.read().map(|a| a.book.ledger + 1).unwrap_or(0);
                    match lake.fetch(next) {
                        Ok(Some(bytes)) => {
                            let mut a = app2.write().unwrap_or_else(|e| e.into_inner());
                            if let Err(e) = a.apply_object(&bytes, false) {
                                eprintln!("sdx serve: apply {next}: {e}");
                                std::thread::sleep(std::time::Duration::from_secs(5));
                            } else if a.book.ledger % 64 == 0 {
                                let _ = a.save(json!({"followed": true}));
                            }
                        }
                        Ok(None) => std::thread::sleep(std::time::Duration::from_secs(1)),
                        Err(e) => {
                            eprintln!("sdx serve: fetch {next}: {e}");
                            std::thread::sleep(std::time::Duration::from_secs(5));
                        }
                    }
                });
            }
            http::serve(&addr, app).map_err(|e| sdx_core::Error::new(sdx_core::error::Code::Io, e.to_string()))?;
        }
        Cmd::Diff { q, horizon, cases, auto, out, lake, workers } => {
            let mut app = open(&q)?;
            let cases: Vec<diff::Case> = match cases {
                Some(p) => {
                    let b = std::fs::read(&p).map_err(|e| sdx_core::Error::io(&p, e))?;
                    serde_json::from_slice(&b).map_err(|e| sdx_core::Error::bad(format!("{}: {e}", p.display())))?
                }
                None => diff::auto_cases(&app, auto),
            };
            std::fs::create_dir_all(&out).map_err(|e| sdx_core::Error::io(&out, e))?;
            let cp = out.join("cases.json");
            std::fs::write(&cp, serde_json::to_vec_pretty(&cases).unwrap_or_default()).map_err(|e| sdx_core::Error::io(&cp, e))?;
            let prov = json!({
                "captured_at_unix": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                "command": std::env::args().collect::<Vec<_>>().join(" "),
                "sdx_version": env!("CARGO_PKG_VERSION"),
                "horizon": horizon,
                "lake": lake,
                "state_ledger_at_start": app.book.ledger,
                "bootstrap_checkpoint": app.bootstrap_checkpoint,
                "network": "pubnet",
                "note": "read-only GETs; Horizon bodies are verbatim; ours computed by sdx at the same Latest-Ledger",
            });
            let pp = out.join("diff.provenance.json");
            std::fs::write(&pp, serde_json::to_vec_pretty(&prov).unwrap_or_default()).map_err(|e| sdx_core::Error::io(&pp, e))?;
            let lake = Lake::new(lake, None);
            let mut client = diff::HorizonClient::new(&horizon);
            let trades_all = snapshot::load_trades(&app.dir, app.book.ledger)?;
            let s = diff::run(&mut app, &mut client, &cases, diff::DiffOptions { out: &out, lake: &lake, workers, trades_all })?;
            app.save(json!({"after_diff": true}))?;
            print(&s);
        }
        Cmd::Status { q } => {
            let app = open(&q)?;
            print(&app.status());
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sdx: {e}");
            match e.code {
                sdx_core::error::Code::BadRequest => ExitCode::from(2),
                _ => ExitCode::from(1),
            }
        }
    }
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
