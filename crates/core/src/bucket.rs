// SPDX-License-Identifier: Apache-2.0
//! State bootstrap from a history archive.
//!
//! A checkpoint's History Archive State (HAS) lists the live bucket list: 11
//! levels, each with a `curr` and a `snap` bucket. Each bucket is a gzip
//! stream of RFC 5531 record-marked `BucketEntry` XDR, sorted by ledger key,
//! and key order puts the entry type first (ACCOUNT=0, TRUSTLINE=1, OFFER=2,
//! DATA=3, CLAIMABLE_BALANCE=4, LIQUIDITY_POOL=5, CONTRACT_DATA=6, ...). So:
//!
//! * we peek the type of each record from its first bytes and fully decode
//!   only OFFER and LIQUIDITY_POOL entries;
//! * we stop reading a bucket at the first record whose type is above
//!   LIQUIDITY_POOL (early exit; Soroban state is never downloaded);
//! * newer buckets shadow older ones (level 0 curr first, level 10 snap last):
//!   the first LIVE/INIT occurrence of a key wins, a DEAD occurrence deletes.
//!
//! Hashes: in full-read mode (`early_exit = false`) the bucket's SHA-256 is
//! checked against its name. In early-exit mode (bootstrap default) nothing is
//! hashed, since the hash covers the whole file; extracts record
//! `hash_verified = false`. The HAS itself is checked
//! against the checkpoint ledger header's `bucket_list_hash` when the ledger
//! header is available (see [`bucket_list_hash`]).

use crate::error::{Code, Error, Result};
use crate::lake::http_get_reader;
use crate::state::{ApplyStats, BookState};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use stellar_xdr::{BucketEntry, LedgerEntry, LedgerEntryData, LedgerKey, Limits, ReadXdr, WriteXdr};

pub const SDF_ARCHIVE: &str = "https://history.stellar.org/prd/core-live/core_live_001";
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

const TYPE_OFFER: i32 = 2;
const TYPE_LIQUIDITY_POOL: i32 = 5;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HasLevel {
    pub curr: String,
    pub snap: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Has {
    pub version: u32,
    pub current_ledger: u32,
    pub network_passphrase: Option<String>,
    pub current_buckets: Vec<HasLevel>,
    #[serde(default)]
    pub hot_archive_buckets: Vec<HasLevel>,
}

impl Has {
    /// Live buckets in shadowing order (newest first), zero hashes skipped.
    pub fn live_buckets(&self) -> Vec<String> {
        let mut v = Vec::new();
        for l in &self.current_buckets {
            for h in [&l.curr, &l.snap] {
                if h != ZERO_HASH {
                    v.push(h.clone());
                }
            }
        }
        v
    }
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    let v = hex::decode(s).map_err(|e| Error::new(Code::Decode, format!("bad hash {s}: {e}")))?;
    v.try_into().map_err(|_| Error::new(Code::Decode, format!("bad hash length {s}")))
}

fn list_hash(levels: &[HasLevel]) -> Result<[u8; 32]> {
    let mut outer = Sha256::new();
    for l in levels {
        let mut inner = Sha256::new();
        inner.update(hex32(&l.curr)?);
        inner.update(hex32(&l.snap)?);
        outer.update(inner.finalize());
    }
    Ok(outer.finalize().into())
}

/// Bucket-list hash the ledger header commits to: SHA-256 over the level hashes
/// (each SHA-256(curr || snap)); from protocol 23 the header holds
/// SHA-256(live list hash || hot-archive list hash).
pub fn bucket_list_hash(has: &Has, protocol: u32) -> Result<[u8; 32]> {
    let live = list_hash(&has.current_buckets)?;
    if protocol < 23 {
        return Ok(live);
    }
    let hot = list_hash(&has.hot_archive_buckets)?;
    let mut h = Sha256::new();
    h.update(live);
    h.update(hot);
    Ok(h.finalize().into())
}

/// Checkpoint ledgers are those with (seq + 1) % 64 == 0.
pub fn checkpoint_at_or_before(seq: u32) -> u32 {
    ((seq + 1) / 64) * 64 - 1
}

pub fn has_url(archive: &str, checkpoint: Option<u32>) -> String {
    match checkpoint {
        None => format!("{archive}/.well-known/stellar-history.json"),
        Some(c) => {
            let h = format!("{c:08x}");
            format!("{archive}/history/{}/{}/{}/history-{h}.json", &h[0..2], &h[2..4], &h[4..6])
        }
    }
}

pub fn bucket_url(archive: &str, hash: &str) -> String {
    format!("{archive}/bucket/{}/{}/{}/bucket-{hash}.xdr.gz", &hash[0..2], &hash[2..4], &hash[4..6])
}

pub fn fetch_has(archive: &str, checkpoint: Option<u32>) -> Result<Has> {
    let url = has_url(archive, checkpoint);
    let mut body = String::new();
    http_get_reader(&url)?.read_to_string(&mut body).map_err(|e| Error::new(Code::Fetch, format!("{url}: {e}")))?;
    serde_json::from_str(&body).map_err(|e| Error::new(Code::Decode, format!("{url}: HAS JSON: {e}")))
}

/// What one bucket contributes: offers/pools, in bucket order.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExtractStats {
    pub hash: String,
    pub records_read: u64,
    pub bytes_uncompressed: u64,
    /// Records per ledger-entry type id (index = type) seen before the early exit.
    pub records_by_type: [u64; 10],
    pub offers_live: u64,
    pub offers_dead: u64,
    pub pools_live: u64,
    pub pools_dead: u64,
    pub early_exit: bool,
    pub hash_verified: bool,
    pub seconds: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Extract {
    pub stats: ExtractStats,
    /// LIVE/INIT offer and pool entries.
    pub live: Vec<LedgerEntry>,
    /// DEAD offer and pool keys.
    pub dead: Vec<LedgerKey>,
}

fn read_record<R: Read>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    let mut mark = [0u8; 4];
    match r.read_exact(&mut mark) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(e) => return Err(e),
    }
    let len = (u32::from_be_bytes(mark) & 0x7fff_ffff) as usize;
    buf.resize(len, 0);
    r.read_exact(buf)?;
    Ok(true)
}

fn be_i32(b: &[u8], at: usize) -> Option<i32> {
    b.get(at..at + 4).map(|x| i32::from_be_bytes([x[0], x[1], x[2], x[3]]))
}

/// A reader that hashes everything read through it.
struct HashingReader<R> {
    inner: R,
    /// Hashing costs as much CPU as gunzip; only done in full-read mode,
    /// the only mode in which the hash can be checked.
    hash: bool,
    hasher: Sha256,
    bytes: u64,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        if self.hash {
            self.hasher.update(&out[..n]);
        }
        self.bytes += n as u64;
        Ok(n)
    }
}

/// Stream one decompressed bucket and extract offers and pools.
pub fn extract_from_reader<R: Read>(hash: &str, raw: R, early_exit: bool) -> Result<Extract> {
    let t0 = std::time::Instant::now();
    let mut r = HashingReader { inner: BufReader::with_capacity(1 << 20, raw), hash: !early_exit, hasher: Sha256::new(), bytes: 0 };
    let mut out = Extract::default();
    out.stats.hash = hash.to_string();
    let mut buf = Vec::with_capacity(4096);
    let dec = |e: std::io::Error| Error::new(Code::Decode, format!("bucket {hash}: {e}"));
    loop {
        if !read_record(&mut r, &mut buf).map_err(dec)? {
            break;
        }
        out.stats.records_read += 1;
        let kind = be_i32(&buf, 0).ok_or_else(|| Error::new(Code::Decode, format!("bucket {hash}: short record")))?;
        let etype = match kind {
            -1 => continue,           // METAENTRY
            0 | 2 => be_i32(&buf, 8), // LIVE/INIT: lastModified u32, then LedgerEntryData type
            1 => be_i32(&buf, 4),     // DEAD: LedgerKey type
            _ => return Err(Error::new(Code::Decode, format!("bucket {hash}: unknown BucketEntry type {kind}"))),
        }
        .ok_or_else(|| Error::new(Code::Decode, format!("bucket {hash}: short record")))?;
        if (0..10).contains(&etype) {
            out.stats.records_by_type[etype as usize] += 1;
        }
        if etype > TYPE_LIQUIDITY_POOL && early_exit {
            out.stats.early_exit = true;
            break;
        }
        if etype != TYPE_OFFER && etype != TYPE_LIQUIDITY_POOL {
            continue;
        }
        let e = BucketEntry::from_xdr(&buf, Limits::none())
            .map_err(|e| Error::new(Code::Decode, format!("bucket {hash}: BucketEntry: {e}")))?;
        match e {
            BucketEntry::Liveentry(le) | BucketEntry::Initentry(le) => {
                match le.data {
                    LedgerEntryData::Offer(_) => out.stats.offers_live += 1,
                    _ => out.stats.pools_live += 1,
                }
                out.live.push(le);
            }
            BucketEntry::Deadentry(k) => {
                match k {
                    LedgerKey::Offer(_) => out.stats.offers_dead += 1,
                    _ => out.stats.pools_dead += 1,
                }
                out.dead.push(k);
            }
            BucketEntry::Metaentry(_) => {}
        }
    }
    out.stats.bytes_uncompressed = r.bytes;
    if !early_exit {
        let got = hex::encode(r.hasher.finalize());
        if got != hash {
            return Err(Error::new(Code::Decode, format!("bucket {hash}: content hash is {got}")));
        }
        out.stats.hash_verified = true;
    }
    out.stats.seconds = t0.elapsed().as_secs_f64();
    Ok(out)
}

fn extract_cache_path(dir: &Path, hash: &str) -> PathBuf {
    dir.join(format!("bucket-{hash}.extract.xdr"))
}

/// Cached extracts: framed BucketEntry XDR (LIVE for entries, DEAD for keys)
/// plus a JSON stats sidecar. Buckets are content-addressed, so an extract is
/// reusable for every later checkpoint that still contains the bucket.
pub fn save_extract(dir: &Path, x: &Extract) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let p = extract_cache_path(dir, &x.stats.hash);
    let mut w = Vec::new();
    for e in &x.live {
        frame(&mut w, &BucketEntry::Liveentry(e.clone()))?;
    }
    for k in &x.dead {
        frame(&mut w, &BucketEntry::Deadentry(k.clone()))?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, &w).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &p).map_err(|e| Error::io(&p, e))?;
    let sp = p.with_extension("json");
    std::fs::write(&sp, serde_json::to_vec_pretty(&x.stats).unwrap_or_default()).map_err(|e| Error::io(&sp, e))
}

pub fn load_extract(dir: &Path, hash: &str) -> Result<Option<Extract>> {
    let p = extract_cache_path(dir, hash);
    let Ok(bytes) = std::fs::read(&p) else { return Ok(None) };
    let Ok(stats_raw) = std::fs::read(p.with_extension("json")) else { return Ok(None) };
    let stats: ExtractStats = serde_json::from_slice(&stats_raw).map_err(|e| Error::new(Code::Decode, format!("{}: {e}", p.display())))?;
    let mut x = Extract { stats, ..Extract::default() };
    let mut r = &bytes[..];
    let mut buf = Vec::new();
    while read_record(&mut r, &mut buf).map_err(|e| Error::io(&p, e))? {
        match BucketEntry::from_xdr(&buf, Limits::none()).map_err(|e| Error::new(Code::Decode, format!("{}: {e}", p.display())))? {
            BucketEntry::Liveentry(e) => x.live.push(e),
            BucketEntry::Deadentry(k) => x.dead.push(k),
            _ => {}
        }
    }
    Ok(Some(x))
}

pub fn frame<T: WriteXdr>(w: &mut Vec<u8>, v: &T) -> Result<()> {
    let b = v.to_xdr(Limits::none()).map_err(|e| Error::new(Code::Decode, format!("encode: {e}")))?;
    w.write_all(&((b.len() as u32) | 0x8000_0000).to_be_bytes()).map_err(|e| Error::new(Code::Io, e.to_string()))?;
    w.extend_from_slice(&b);
    Ok(())
}

/// Download (or load from cache) and extract one bucket.
pub fn extract_bucket(archive: &str, hash: &str, cache: Option<&Path>, early_exit: bool) -> Result<Extract> {
    if let Some(dir) = cache {
        if let Some(x) = load_extract(dir, hash)? {
            return Ok(x);
        }
    }
    let x = if let Some(local) = archive.strip_prefix("file://") {
        let p = PathBuf::from(local).join(bucket_url("", hash).trim_start_matches('/'));
        let f = std::fs::File::open(&p).map_err(|e| Error::io(&p, e))?;
        extract_from_reader(hash, GzDecoder::new(BufReader::new(f)), early_exit)?
    } else {
        let url = bucket_url(archive, hash);
        extract_from_reader(hash, GzDecoder::new(crate::lake::ResumingReader::open(&url)?), early_exit)?
    };
    if let Some(dir) = cache {
        save_extract(dir, &x)?;
    }
    Ok(x)
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct BootstrapReport {
    pub checkpoint: u32,
    pub buckets: Vec<ExtractStats>,
    pub offers: usize,
    pub pools: usize,
    pub assets: usize,
    pub seconds: f64,
    pub peak_rss_kib: Option<u64>,
}

/// Merge extracts (newest first) into a state at the checkpoint ledger.
pub fn assemble(checkpoint: u32, extracts: &[Extract]) -> Result<BookState> {
    let mut seen_offers: HashSet<i64> = HashSet::new();
    let mut seen_pools: HashSet<[u8; 32]> = HashSet::new();
    let mut state = BookState::new();
    let mut stats = ApplyStats::default();
    for x in extracts {
        // Within one bucket a key appears at most once, so order inside is irrelevant.
        for k in &x.dead {
            match k {
                LedgerKey::Offer(o) => {
                    seen_offers.insert(o.offer_id);
                }
                LedgerKey::LiquidityPool(p) => {
                    seen_pools.insert(p.liquidity_pool_id.0 .0);
                }
                _ => {}
            }
        }
        for e in &x.live {
            let fresh = match &e.data {
                LedgerEntryData::Offer(o) => seen_offers.insert(o.offer_id),
                LedgerEntryData::LiquidityPool(p) => seen_pools.insert(p.liquidity_pool_id.0 .0),
                _ => false,
            };
            if fresh {
                state.upsert(e, &mut stats)?;
            }
        }
    }
    state.ledger = checkpoint;
    Ok(state)
}

/// Peak resident set size of this process (Linux `VmHWM`), KiB.
pub fn peak_rss_kib() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines().find(|l| l.starts_with("VmHWM:"))?.split_whitespace().nth(1)?.parse().ok()
}

/// Full bootstrap: fetch HAS, extract every live bucket with `workers` threads,
/// assemble. `on_bucket` is called as each bucket finishes (progress output).
pub fn bootstrap(
    archive: &str,
    checkpoint: Option<u32>,
    cache: Option<&Path>,
    workers: usize,
    on_bucket: &(dyn Fn(&ExtractStats) + Sync),
) -> Result<(BookState, Has, BootstrapReport)> {
    let t0 = std::time::Instant::now();
    let has = fetch_has(archive, checkpoint)?;
    let hashes = has.live_buckets();
    let results: Vec<std::sync::Mutex<Option<Result<Extract>>>> = hashes.iter().map(|_| std::sync::Mutex::new(None)).collect();
    // Largest buckets first so the long downloads start immediately.
    let order: Vec<usize> = (0..hashes.len()).rev().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(&idx) = order.get(i) else { break };
                let r = extract_bucket(archive, &hashes[idx], cache, true);
                if let Ok(x) = &r {
                    on_bucket(&x.stats);
                }
                *results[idx].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
            });
        }
    });
    let mut extracts = Vec::with_capacity(hashes.len());
    for m in results {
        let r = m.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(|| Err(Error::new(Code::Fetch, "bucket not processed")));
        extracts.push(r?);
    }
    let state = assemble(has.current_ledger, &extracts)?;
    let report = BootstrapReport {
        checkpoint: has.current_ledger,
        buckets: extracts.iter().map(|x| x.stats.clone()).collect(),
        offers: state.offer_count(),
        pools: state.pool_count(),
        assets: state.total_assets(),
        seconds: t0.elapsed().as_secs_f64(),
        peak_rss_kib: peak_rss_kib(),
    };
    Ok((state, has, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testutil::*;
    use stellar_xdr::Asset;

    #[test]
    fn urls_and_checkpoints() {
        assert_eq!(checkpoint_at_or_before(65_290_111), 65_290_111);
        assert_eq!(checkpoint_at_or_before(65_290_175), 65_290_175);
        assert_eq!(checkpoint_at_or_before(65_290_174), 65_290_111);
        assert_eq!(has_url("A", Some(65_290_111)), "A/history/03/e4/3f/history-03e43f7f.json");
        assert_eq!(bucket_url("A", "1c34d5d65f72"), "A/bucket/1c/34/d5/bucket-1c34d5d65f72.xdr.gz");
    }

    #[test]
    fn extract_shadowing_and_early_exit() {
        let usd = credit("USD", 9);
        let o1 = offer(1, 1, Asset::Native, usd.clone(), 10, 1, 1);
        let o1_old = offer(1, 1, Asset::Native, usd.clone(), 99, 1, 1);
        let o2 = offer(2, 1, Asset::Native, usd.clone(), 20, 1, 1);
        let p = pool(5, Asset::Native, usd.clone(), 1, 2);
        // newest bucket: o1 live, o2 dead
        let mut newest = Vec::new();
        frame(&mut newest, &BucketEntry::Liveentry(o1.clone())).unwrap();
        frame(&mut newest, &BucketEntry::Deadentry(o2.to_key())).unwrap();
        // older bucket: o1 (older value), o2 live, pool; then a contract-data-typed record to trigger early exit
        let mut older = Vec::new();
        frame(&mut older, &BucketEntry::Liveentry(o1_old)).unwrap();
        frame(&mut older, &BucketEntry::Initentry(o2)).unwrap();
        frame(&mut older, &BucketEntry::Liveentry(p)).unwrap();
        // fake record with entry type 6 (CONTRACT_DATA) in the peeked position; never decoded
        older.extend_from_slice(&(12u32 | 0x8000_0000).to_be_bytes());
        older.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 6]);
        let h_new = hex::encode(Sha256::digest(&newest));
        let x_new = extract_from_reader(&h_new, &newest[..], false).unwrap();
        assert!(x_new.stats.hash_verified);
        let x_old = extract_from_reader("unverifiable", &older[..], true).unwrap();
        assert!(x_old.stats.early_exit);
        assert_eq!(x_old.stats.records_by_type[6], 1);
        assert!(extract_from_reader("deadbeef", &newest[..], false).is_err(), "hash mismatch must fail");
        assert!(!extract_from_reader("deadbeef", &newest[..], true).unwrap().stats.hash_verified, "early-exit mode does not hash");
        let s = assemble(63, &[x_new, x_old]).unwrap();
        assert_eq!(s.offer_count(), 1);
        assert_eq!(s.offer(1).unwrap().amount, 10);
        assert_eq!(s.pool_count(), 1);
        assert_eq!(s.ledger, 63);
    }

    #[test]
    fn extract_cache_round_trip() {
        let dir = std::env::temp_dir().join(format!("sdx-extract-{}", std::process::id()));
        let usd = credit("USD", 9);
        let x = Extract {
            stats: ExtractStats { hash: "ab".repeat(32), ..ExtractStats::default() },
            live: vec![offer(3, 1, Asset::Native, usd.clone(), 5, 1, 2)],
            dead: vec![offer(4, 1, Asset::Native, usd, 5, 1, 2).to_key()],
        };
        save_extract(&dir, &x).unwrap();
        let y = load_extract(&dir, &x.stats.hash).unwrap().unwrap();
        assert_eq!(y.live, x.live);
        assert_eq!(y.dead, x.dead);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bucket_list_hash_layout() {
        let lvl = |c: &str, s: &str| HasLevel { curr: c.repeat(64), snap: s.repeat(64) };
        let has = Has {
            version: 2,
            current_ledger: 63,
            network_passphrase: None,
            current_buckets: vec![lvl("1", "2"), lvl("0", "0")],
            hot_archive_buckets: vec![lvl("0", "0")],
        };
        let a = bucket_list_hash(&has, 22).unwrap();
        let b = bucket_list_hash(&has, 23).unwrap();
        assert_ne!(a, b);
    }
}
