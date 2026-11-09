// SPDX-License-Identifier: Apache-2.0
//! sdex-orderbook core: rebuild the SDEX order book, classic AMM pools,
//! trades and trade aggregations from ledger data (history-archive buckets +
//! Galexie `LedgerCloseMeta`), and answer Horizon-shaped queries over them.
//!
//! Module map (ARCHITECTURE.md has the data flow):
//! * [`asset`], [`amount`], [`price`], [`pool_math`]: value types and the exact
//!   integer arithmetic Horizon uses (ported from go-stellar-sdk, see NOTICE).
//! * [`state`]: in-memory offers + pools with per-pair sorted books.
//! * [`bucket`]: history-archive state bootstrap (HAS + bucket streaming).
//! * [`lake`] + [`ledger`]: Galexie object fetch/decode and per-ledger apply.
//! * [`trades`] + [`aggregate`]: ClaimAtom-derived trades and OHLCV buckets.
//! * [`book`], [`paths`]: order-book summary and path finding.
//! * [`horizon`]: Horizon-compatible JSON rendering and query parsing.
//! * [`snapshot`]: on-disk state (offers/pools XDR + trades JSONL).

pub mod aggregate;
pub mod amount;
pub mod asset;
pub mod book;
pub mod bucket;
pub mod error;
pub mod horizon;
pub mod lake;
pub mod ledger;
pub mod paths;
pub mod pool_math;
pub mod price;
pub mod snapshot;
pub mod state;
pub mod trades;

pub use error::{Error, Result};
