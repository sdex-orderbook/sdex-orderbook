// SPDX-License-Identifier: Apache-2.0
//! On-disk state directory (SPEC section 4):
//!
//! ```text
//! <dir>/state.json        manifest: format, ledger, close_time, counts, sha256, source
//! <dir>/state.xdr.zst     zstd( framed LedgerEntry XDR: offers by id, then pools by id )
//! <dir>/trades.jsonl      one Trade per line, appended by `replay`
//! <dir>/replay.jsonl      one line of apply statistics per replayed ledger
//! ```

use crate::bucket::frame;
use crate::error::{Code, Error, Result};
use crate::state::{ApplyStats, BookState};
use crate::trades::Trade;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Write};
use std::path::Path;
use stellar_xdr::{LedgerEntry, Limits, ReadXdr};

pub const FORMAT: &str = "sdx-state/1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub format: String,
    pub network: String,
    pub ledger: u32,
    pub close_time: u64,
    pub offers: usize,
    pub pools: usize,
    /// SHA-256 of the uncompressed framed XDR.
    pub sha256: String,
    /// Checkpoint the state was bootstrapped from.
    pub bootstrap_checkpoint: u32,
    pub source: serde_json::Value,
}

/// Canonical bytes of a state (framed XDR, offers then pools, by id).
pub fn state_bytes(state: &BookState) -> Result<Vec<u8>> {
    let mut w = Vec::new();
    for o in state.offers_sorted() {
        frame(&mut w, &o.entry)?;
    }
    for p in state.pools_sorted() {
        frame(&mut w, &p.entry)?;
    }
    Ok(w)
}

pub fn state_hash(state: &BookState) -> Result<String> {
    Ok(hex::encode(Sha256::digest(state_bytes(state)?)))
}

pub fn save(dir: &Path, state: &BookState, bootstrap_checkpoint: u32, source: serde_json::Value) -> Result<Manifest> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let raw = state_bytes(state)?;
    let sha = hex::encode(Sha256::digest(&raw));
    let z = zstd::stream::encode_all(&raw[..], 3).map_err(|e| Error::new(Code::Io, format!("zstd: {e}")))?;
    let p = dir.join("state.xdr.zst");
    let tmp = dir.join("state.xdr.zst.tmp");
    std::fs::write(&tmp, &z).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &p).map_err(|e| Error::io(&p, e))?;
    let m = Manifest {
        format: FORMAT.into(),
        network: "pubnet".into(),
        ledger: state.ledger,
        close_time: state.close_time,
        offers: state.offer_count(),
        pools: state.pool_count(),
        sha256: sha,
        bootstrap_checkpoint,
        source,
    };
    let mp = dir.join("state.json");
    std::fs::write(&mp, serde_json::to_vec_pretty(&m).unwrap_or_default()).map_err(|e| Error::io(&mp, e))?;
    Ok(m)
}

pub fn load_manifest(dir: &Path) -> Result<Manifest> {
    let mp = dir.join("state.json");
    let b = std::fs::read(&mp).map_err(|e| Error::io(&mp, e))?;
    serde_json::from_slice(&b).map_err(|e| Error::new(Code::Decode, format!("{}: {e}", mp.display())))
}

/// Load a state directory and verify its hash.
pub fn load(dir: &Path) -> Result<(BookState, Manifest)> {
    let m = load_manifest(dir)?;
    if m.format != FORMAT {
        return Err(Error::new(Code::Decode, format!("unsupported state format {}", m.format)));
    }
    let p = dir.join("state.xdr.zst");
    let z = std::fs::read(&p).map_err(|e| Error::io(&p, e))?;
    let raw = zstd::stream::decode_all(&z[..]).map_err(|e| Error::new(Code::Decode, format!("{}: zstd: {e}", p.display())))?;
    let sha = hex::encode(Sha256::digest(&raw));
    if sha != m.sha256 {
        return Err(Error::new(Code::Consistency, format!("{}: sha256 {sha} != manifest {}", p.display(), m.sha256)));
    }
    let mut state = BookState::new();
    let mut stats = ApplyStats::default();
    let mut r = &raw[..];
    while r.len() >= 4 {
        let len = (u32::from_be_bytes([r[0], r[1], r[2], r[3]]) & 0x7fff_ffff) as usize;
        if r.len() < 4 + len {
            return Err(Error::new(Code::Decode, format!("{}: truncated record", p.display())));
        }
        let e =
            LedgerEntry::from_xdr(&r[4..4 + len], Limits::none()).map_err(|e| Error::new(Code::Decode, format!("{}: {e}", p.display())))?;
        state.upsert(&e, &mut stats)?;
        r = &r[4 + len..];
    }
    state.ledger = m.ledger;
    state.close_time = m.close_time;
    Ok((state, m))
}

pub fn append_trades(dir: &Path, trades: &[Trade]) -> Result<()> {
    append_lines(&dir.join("trades.jsonl"), trades.iter().map(|t| serde_json::to_string(t).unwrap_or_default()))
}

pub fn append_lines(path: &Path, lines: impl Iterator<Item = String>) -> Result<()> {
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e| Error::io(path, e))?;
    for l in lines {
        writeln!(f, "{l}").map_err(|e| Error::io(path, e))?;
    }
    Ok(())
}

/// Trades with `ledger <= up_to` (lines past the state's ledger are ignored,
/// so an interrupted replay never double counts).
pub fn load_trades(dir: &Path, up_to: u32) -> Result<Vec<Trade>> {
    let p = dir.join("trades.jsonl");
    let Ok(f) = std::fs::File::open(&p) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in std::io::BufReader::new(f).lines() {
        let line = line.map_err(|e| Error::io(&p, e))?;
        if line.trim().is_empty() {
            continue;
        }
        let t: Trade = serde_json::from_str(&line).map_err(|e| Error::new(Code::Decode, format!("{}: {e}", p.display())))?;
        if t.ledger <= up_to && seen.insert((t.op_id, t.order)) {
            out.push(t);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::*;
    use stellar_xdr::Asset;

    #[test]
    fn save_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("sdx-snap-{}", std::process::id()));
        let mut s = BookState::new();
        let mut st = ApplyStats::default();
        let usd = credit("USD", 9);
        s.upsert(&offer(9, 1, Asset::Native, usd.clone(), 5, 1, 2), &mut st).unwrap();
        s.upsert(&offer(3, 1, usd.clone(), Asset::Native, 7, 2, 1), &mut st).unwrap();
        s.upsert(&pool(1, Asset::Native, usd, 10, 20), &mut st).unwrap();
        s.ledger = 100;
        s.close_time = 1234;
        let m = save(&dir, &s, 63, serde_json::json!({"test": true})).unwrap();
        let (t, m2) = load(&dir).unwrap();
        assert_eq!(m, m2);
        assert_eq!(t.offer_count(), 2);
        assert_eq!(t.pool_count(), 1);
        assert_eq!(state_hash(&t).unwrap(), m.sha256);
        // tamper -> hash failure
        let p = dir.join("state.json");
        let mut bad = m.clone();
        bad.sha256 = "00".repeat(32);
        std::fs::write(&p, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(load(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
