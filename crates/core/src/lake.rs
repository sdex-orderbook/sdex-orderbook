// SPDX-License-Identifier: Apache-2.0
//! Read-only access to the public Galexie ledger lake (AWS Open Data,
//! `v1.1/stellar/ledgers/pubnet`, one zstd `LedgerCloseMetaBatch` per ledger,
//! 64,000 ledgers per partition) and plain HTTP GET helpers.
//!
//! A base of `file:///dir` reads a local mirror with the bucket layout; the
//! offline tests use that. Fetched objects can be cached by key.

use crate::error::{Code, Error, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use stellar_xdr::{LedgerCloseMeta, LedgerCloseMetaBatch, Limits, ReadXdr};

pub const LAKE_URL: &str = "https://aws-public-blockchain.s3.amazonaws.com";
pub const PUBNET_PREFIX: &str = "v1.1/stellar/ledgers/pubnet";
pub const LEDGERS_PER_PARTITION: u32 = 64_000;
pub const USER_AGENT: &str = concat!("sik-sdex-orderbook/", env!("CARGO_PKG_VERSION"), " (read-only research tool)");

fn rev(x: u32) -> u32 {
    u32::MAX - x
}

pub fn object_key(seq: u32) -> String {
    let start = seq / LEDGERS_PER_PARTITION * LEDGERS_PER_PARTITION;
    let end = start + LEDGERS_PER_PARTITION - 1;
    format!("{PUBNET_PREFIX}/{:08X}--{start}-{end}/{:08X}--{seq}.xdr.zst", rev(start), rev(seq))
}

pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(120)).user_agent(USER_AGENT).build()
}

thread_local! {
    static AGENT: ureq::Agent = agent();
}

/// GET with 3 tries (1 s, 2 s back-off; honours Retry-After on 429/5xx).
/// `Ok(None)` on 404.
pub fn http_get_opt(url: &str) -> Result<Option<Box<dyn Read + Send + Sync>>> {
    http_get_from(url, 0)
}

fn http_get_from(url: &str, offset: u64) -> Result<Option<Box<dyn Read + Send + Sync>>> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        let r = AGENT.with(|a| {
            let mut req = a.get(url);
            if offset > 0 {
                req = req.set("Range", &format!("bytes={offset}-"));
            }
            req.call().map_err(Box::new)
        });
        if let Ok(resp) = &r {
            if offset > 0 && resp.status() != 206 {
                return Err(Error::new(Code::Fetch, format!("{url}: server ignored Range (status {})", resp.status())));
            }
        }
        match r {
            Ok(resp) => return Ok(Some(resp.into_reader())),
            Err(e) if matches!(*e, ureq::Error::Status(404, _)) => return Ok(None),
            Err(e) if matches!(*e, ureq::Error::Status(..)) => {
                let ureq::Error::Status(code, resp) = *e else { unreachable!() };
                let wait = resp.header("Retry-After").and_then(|v| v.parse::<u64>().ok()).unwrap_or(attempt + 1);
                last = format!("HTTP {code}");
                if code == 429 || code >= 500 {
                    std::thread::sleep(Duration::from_secs(wait.min(30)));
                    continue;
                }
                return Err(Error::new(Code::Fetch, format!("{url}: {last}")));
            }
            Err(e) => {
                last = e.to_string();
                std::thread::sleep(Duration::from_secs(attempt + 1));
            }
        }
    }
    Err(Error::new(Code::Fetch, format!("{url}: {last} (after 3 tries)")))
}

pub fn http_get_reader(url: &str) -> Result<Box<dyn Read + Send + Sync>> {
    http_get_opt(url)?.ok_or_else(|| Error::new(Code::NotFound, format!("{url}: 404")))
}

/// A body reader that survives dropped connections on long downloads: on a
/// read error it re-requests the rest with `Range: bytes=<offset>-` (up to
/// `MAX_RESUMES` times) and continues the same byte stream, so a streaming
/// gzip decoder on top never notices. Measured need: the 2.5 GB level-10
/// bucket was cut mid-stream on 2026-11-04 ("response body closed").
pub struct ResumingReader {
    url: String,
    offset: u64,
    inner: Box<dyn Read + Send + Sync>,
    pub resumes: u32,
}

const MAX_RESUMES: u32 = 20;

impl ResumingReader {
    pub fn open(url: &str) -> Result<ResumingReader> {
        Ok(ResumingReader { url: url.to_string(), offset: 0, inner: http_get_reader(url)?, resumes: 0 })
    }
}

impl Read for ResumingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.inner.read(buf) {
                Ok(n) => {
                    self.offset += n as u64;
                    return Ok(n);
                }
                Err(e) if self.resumes < MAX_RESUMES => {
                    self.resumes += 1;
                    eprintln!("sdx: {}: {e}; resuming at byte {} (resume {})", self.url, self.offset, self.resumes);
                    std::thread::sleep(Duration::from_secs(2));
                    match http_get_from(&self.url, self.offset) {
                        Ok(Some(r)) => self.inner = r,
                        Ok(None) => return Err(std::io::Error::other(format!("{}: 404 on resume", self.url))),
                        Err(x) => return Err(std::io::Error::other(x.to_string())),
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn http_get_bytes(url: &str) -> Result<Option<Vec<u8>>> {
    let Some(mut r) = http_get_opt(url)? else { return Ok(None) };
    let mut v = Vec::new();
    r.read_to_end(&mut v).map_err(|e| Error::new(Code::Fetch, format!("{url}: {e}")))?;
    Ok(Some(v))
}

type Slot = std::sync::Mutex<Option<Result<Option<Vec<u8>>>>>;

#[derive(Debug, Clone)]
pub struct Lake {
    /// `https://...` bucket root or `file:///abs/dir` mirror root.
    pub base: String,
    pub cache: Option<PathBuf>,
}

impl Lake {
    pub fn new(base: impl Into<String>, cache: Option<PathBuf>) -> Lake {
        Lake { base: base.into().trim_end_matches('/').to_string(), cache }
    }

    pub fn public(cache: Option<PathBuf>) -> Lake {
        Lake::new(LAKE_URL, cache)
    }

    /// Compressed object for one ledger; `Ok(None)` if not (yet) in the lake.
    pub fn fetch(&self, seq: u32) -> Result<Option<Vec<u8>>> {
        let key = object_key(seq);
        if let Some(c) = &self.cache {
            if let Ok(b) = std::fs::read(c.join(&key)) {
                return Ok(Some(b));
            }
        }
        let bytes = if let Some(root) = self.base.strip_prefix("file://") {
            match std::fs::read(Path::new(root).join(&key)) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(Error::io(Path::new(root), e)),
            }
        } else {
            match http_get_bytes(&format!("{}/{key}", self.base))? {
                Some(b) => b,
                None => return Ok(None),
            }
        };
        if let Some(c) = &self.cache {
            let p = c.join(&key);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).map_err(|e| Error::io(d, e))?;
            }
            let tmp = p.with_extension(format!("tmp{}", std::process::id()));
            std::fs::write(&tmp, &bytes).map_err(|e| Error::io(&tmp, e))?;
            std::fs::rename(&tmp, &p).map_err(|e| Error::io(&p, e))?;
        }
        Ok(Some(bytes))
    }

    /// Fetch `seqs` with up to `workers` concurrent requests; results in input order.
    pub fn fetch_many(&self, seqs: &[u32], workers: usize) -> Vec<Result<Option<Vec<u8>>>> {
        let slots: Vec<Slot> = seqs.iter().map(|_| std::sync::Mutex::new(None)).collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..workers.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(seq) = seqs.get(i) else { break };
                    let r = self.fetch(*seq);
                    *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                });
            }
        });
        slots.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or(Ok(None))).collect()
    }
}

/// zstd-decompress and decode one lake object into its ledgers.
pub fn decode(compressed: &[u8]) -> Result<Vec<LedgerCloseMeta>> {
    let raw = zstd::stream::decode_all(compressed).map_err(|e| Error::new(Code::Decode, format!("zstd: {e}")))?;
    let batch =
        LedgerCloseMetaBatch::from_xdr(&raw, Limits::none()).map_err(|e| Error::new(Code::Decode, format!("LedgerCloseMetaBatch: {e}")))?;
    Ok(batch.ledger_close_metas.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_the_public_bucket() {
        // Keys listed/HEAD-ed in the bucket on 2026-10-31 (holders-at-ledger) and 2026-11-03.
        assert_eq!(object_key(2), "v1.1/stellar/ledgers/pubnet/FFFFFFFF--0-63999/FFFFFFFD--2.xdr.zst");
        assert_eq!(object_key(65_234_881), "v1.1/stellar/ledgers/pubnet/FC1CE1FF--65216000-65279999/FC1C983E--65234881.xdr.zst");
        assert!(object_key(65_290_112).ends_with("/FC1BC07F--65290112.xdr.zst"));
    }

    #[test]
    fn missing_local_object_is_none() {
        let dir = std::env::temp_dir().join(format!("sdx-empty-lake-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lake = Lake::new(format!("file://{}", dir.display()), None);
        assert!(lake.fetch(5).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
