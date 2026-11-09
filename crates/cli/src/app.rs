// SPDX-License-Identifier: Apache-2.0
//! Application state shared by the CLI commands, the HTTP server and the
//! differential checker: a [`BookState`], the minute rows built from the
//! trades replayed so far, and the replay loop.

use sdx_core::aggregate::{Minutes, DEFAULT_SLIPPAGE_FILTER};
use sdx_core::error::{Code, Error, Result};
use sdx_core::lake::{self, Lake};
use sdx_core::ledger;
use sdx_core::snapshot;
use sdx_core::state::{ApplyStats, BookState};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct App {
    pub book: BookState,
    pub minutes: Minutes,
    pub max_path_length: usize,
    pub include_pools: bool,
    pub dir: PathBuf,
    pub bootstrap_checkpoint: u32,
    /// First ledger whose trades are included (bootstrap checkpoint + 1).
    pub trades_from: u32,
    pub totals: ApplyStats,
    pub trade_errors: u64,
}

impl App {
    pub fn open(dir: &Path, max_path_length: usize, include_pools: bool) -> Result<App> {
        let (book, m) = snapshot::load(dir)?;
        let trades = snapshot::load_trades(dir, book.ledger)?;
        let minutes = Minutes::build(&trades, DEFAULT_SLIPPAGE_FILTER);
        Ok(App {
            book,
            minutes,
            max_path_length,
            include_pools,
            dir: dir.to_path_buf(),
            bootstrap_checkpoint: m.bootstrap_checkpoint,
            trades_from: m.bootstrap_checkpoint + 1,
            totals: ApplyStats::default(),
            trade_errors: 0,
        })
    }

    pub fn status(&self) -> Value {
        json!({
            "service": "sdex-orderbook",
            "version": env!("CARGO_PKG_VERSION"),
            "network": "pubnet",
            "ledger": self.book.ledger,
            "ledger_close_time": self.book.close_time,
            "offers": self.book.offer_count(),
            "liquidity_pools": self.book.pool_count(),
            "assets_interned": self.book.total_assets(),
            "bootstrap_checkpoint": self.bootstrap_checkpoint,
            "trades_cover_ledgers": [self.trades_from, self.book.ledger],
            "trades_loaded": self.minutes.trades,
            "trades_filtered_by_rounding_slippage": self.minutes.filtered,
            "max_path_length": self.max_path_length,
            "include_pools": self.include_pools,
        })
    }

    pub fn save(&self, source: Value) -> Result<snapshot::Manifest> {
        snapshot::save(&self.dir, &self.book, self.bootstrap_checkpoint, source)
    }

    /// Apply one decoded ledger object; persist trades and stats lines.
    pub fn apply_object(&mut self, bytes: &[u8], strict: bool) -> Result<Vec<ledger::Outcome>> {
        self.apply_metas(lake::decode(bytes)?, strict)
    }

    /// Apply already-decoded ledgers (decoding is the expensive part, so
    /// `replay_to` decodes a whole fetch batch in parallel first).
    pub fn apply_metas(&mut self, metas: Vec<stellar_xdr::LedgerCloseMeta>, strict: bool) -> Result<Vec<ledger::Outcome>> {
        let mut outs = Vec::new();
        for meta in metas {
            let seq = ledger::view(&meta).sequence();
            if seq <= self.book.ledger {
                continue;
            }
            let t0 = std::time::Instant::now();
            let out = ledger::apply(&mut self.book, &meta, strict)?;
            snapshot::append_trades(&self.dir, &out.trades)?;
            self.minutes.add(&out.trades, DEFAULT_SLIPPAGE_FILTER);
            self.totals.add(&out.stats);
            self.trade_errors += out.trade_errors.len() as u64;
            let line = json!({
                "ledger": out.sequence,
                "close_time": out.close_time,
                "trades": out.trades.len(),
                "trade_errors": out.trade_errors,
                "stats": out.stats,
                "offers": self.book.offer_count(),
                "pools": self.book.pool_count(),
                "apply_ms": t0.elapsed().as_secs_f64() * 1000.0,
            });
            snapshot::append_lines(&self.dir.join("replay.jsonl"), std::iter::once(line.to_string()))?;
            outs.push(out);
        }
        Ok(outs)
    }

    /// Replay ledgers until `target` (inclusive). Missing lake objects are waited
    /// for (up to `wait_secs`) since the lake trails the network by seconds.
    pub fn replay_to(
        &mut self,
        lake: &Lake,
        target: u32,
        workers: usize,
        strict: bool,
        wait_secs: u64,
        mut progress: impl FnMut(&App),
    ) -> Result<()> {
        let mut waited = 0u64;
        while self.book.ledger < target {
            let next = self.book.ledger + 1;
            let last = target.min(next + (workers.max(1) as u32) * 2 - 1);
            let seqs: Vec<u32> = (next..=last).collect();
            let fetched = lake.fetch_many(&seqs, workers);
            // decode the batch in parallel, keeping order
            let decoded: Vec<Result<Option<Vec<stellar_xdr::LedgerCloseMeta>>>> = std::thread::scope(|sc| {
                let hs: Vec<_> =
                    fetched.into_iter().map(|r| sc.spawn(move || r.and_then(|o| o.map(|b| lake::decode(&b)).transpose()))).collect();
                hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Error::new(Code::Decode, "decode thread panicked")))).collect()
            });
            let mut advanced = false;
            for (seq, r) in seqs.iter().zip(decoded) {
                match r? {
                    Some(metas) => {
                        self.apply_metas(metas, strict)?;
                        advanced = true;
                    }
                    None => {
                        if *seq == self.book.ledger + 1 && waited < wait_secs {
                            break;
                        }
                        if *seq == self.book.ledger + 1 {
                            return Err(Error::new(Code::Fetch, format!("ledger {seq} not in the lake after {wait_secs}s")));
                        }
                        break;
                    }
                }
            }
            if advanced {
                waited = 0;
                progress(self);
            } else {
                std::thread::sleep(std::time::Duration::from_secs(1));
                waited += 1;
            }
        }
        Ok(())
    }
}
