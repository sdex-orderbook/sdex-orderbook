// SPDX-License-Identifier: Apache-2.0
//! Minimal read-only HTTP/1.1 server on `std::net` (GET only, one thread per
//! connection, `Connection: close`). Local use; bounded (timeouts, line/header caps, connection and
//! in-flight caps, SDX-029) but not hardened for the internet: put a reverse
//! proxy with rate limits in front before hosting (DEPLOY.md).

use crate::app::App;
use sdx_core::error::{Code, Error};
use sdx_core::horizon;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Read-only GET API, so any origin may read it (Horizon sends the same
/// `Access-Control-Allow-Origin: *`). `Latest-Ledger` is exposed so a browser
/// page (the `web/` demo's live mode) can show which ledger answered. SDX-023.
pub const CORS_HEADERS: &str = "Access-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: Latest-Ledger\r\n";

pub struct Response {
    pub status: u16,
    pub body: String,
    pub latest_ledger: u32,
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        414 => "URI Too Long",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

/// Route one request (`target` = path + optional query). Pure; tested directly.
pub fn route(app: &App, target: &str) -> Response {
    let (path, qs) = target.split_once('?').unwrap_or((target, ""));
    let p = horizon::parse_query(qs);
    let ledger = app.book.ledger;
    let result = match path {
        "/" => Ok(app.status()),
        "/order_book" => horizon::order_book(&app.book, &p),
        "/paths/strict-send" => horizon::strict_send(&app.book, &p, app.max_path_length, app.include_pools),
        "/paths/strict-receive" | "/paths" => horizon::strict_receive(&app.book, &p, app.max_path_length, app.include_pools),
        "/trade_aggregations" => horizon::trade_aggregations(&app.minutes, &p, target),
        _ => Err(Error::new(Code::NotFound, format!("no route for {path}"))),
    };
    match result {
        Ok(v) => Response { status: 200, body: serde_json::to_string_pretty(&v).unwrap_or_default(), latest_ledger: ledger },
        Err(e) => Response {
            status: e.code.http_status(),
            body: serde_json::to_string_pretty(&horizon::problem(&e)).unwrap_or_default(),
            latest_ledger: ledger,
        },
    }
}

/// Hardening limits (SDX-029). They bound a hostile or stuck client; they do
/// not make this a public-internet server (see DEPLOY.md: reverse proxy first).
pub const MAX_LINE_BYTES: u64 = 8 * 1024;
pub const MAX_HEADER_LINES: usize = 64;
pub const IO_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_CONNECTIONS: usize = 64;
/// Concurrent route computations (each holds the state read lock; `/paths` is
/// the expensive one). Requests over the cap get 503 instead of queueing.
pub const MAX_INFLIGHT_ROUTES: usize = 4;

/// Read one `\n`-terminated line of at most `MAX_LINE_BYTES`. `Ok(None)` means
/// the line was longer than the cap (or EOF before any byte).
fn read_bounded_line<R: BufRead>(r: &mut R, out: &mut Vec<u8>) -> std::io::Result<Option<usize>> {
    out.clear();
    let n = r.by_ref().take(MAX_LINE_BYTES).read_until(b'\n', out)?;
    if n as u64 == MAX_LINE_BYTES && out.last() != Some(&b'\n') {
        return Ok(None);
    }
    Ok(Some(n))
}

/// Parse the request head. Returns the request line, or a status to answer with.
fn read_head<R: BufRead>(r: &mut R) -> Result<String, u16> {
    let mut buf = Vec::new();
    let line = match read_bounded_line(r, &mut buf) {
        Ok(Some(n)) if n > 0 => String::from_utf8_lossy(&buf).into_owned(),
        Ok(Some(_)) => return Err(400),
        Ok(None) => return Err(414),
        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Err(408),
        Err(_) => return Err(400),
    };
    for _ in 0..MAX_HEADER_LINES {
        match read_bounded_line(r, &mut buf) {
            Ok(Some(0)) => return Ok(line),
            Ok(Some(_)) if buf == b"\r\n" || buf == b"\n" => return Ok(line),
            Ok(Some(_)) => {}
            Ok(None) => return Err(431),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Err(408),
            Err(_) => return Err(400),
        }
    }
    Err(431)
}

fn write_response(mut w: TcpStream, resp: &Response) -> std::io::Result<()> {
    write!(
        w,
        "HTTP/1.1 {} {}\r\nContent-Type: application/hal+json; charset=utf-8\r\nLatest-Ledger: {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp.status,
        reason(resp.status),
        resp.latest_ledger,
        CORS_HEADERS,
        resp.body.len(),
        resp.body
    )?;
    w.flush()
}

fn simple(status: u16) -> Response {
    Response { status, body: format!("{{\"status\":{status}}}"), latest_ledger: 0 }
}

/// RAII slot in a counter; `None` when the cap is already reached.
struct Slot<'a>(&'a AtomicUsize);
impl<'a> Slot<'a> {
    fn take(c: &'a AtomicUsize, cap: usize) -> Option<Self> {
        if c.fetch_add(1, Ordering::AcqRel) >= cap {
            c.fetch_sub(1, Ordering::AcqRel);
            None
        } else {
            Some(Slot(c))
        }
    }
}
impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

static INFLIGHT: AtomicUsize = AtomicUsize::new(0);

fn handle(stream: TcpStream, app: &RwLock<App>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut refused = false;
    let resp = match read_head(&mut reader) {
        Err(status) => {
            refused = true;
            simple(status)
        }
        Ok(line) => {
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("/");
            if method != "GET" {
                simple(405)
            } else if let Some(_slot) = Slot::take(&INFLIGHT, MAX_INFLIGHT_ROUTES) {
                let guard = app.read().unwrap_or_else(|e| e.into_inner());
                route(&guard, target)
            } else {
                simple(503)
            }
        }
    };
    let closer = stream.try_clone()?;
    write_response(stream, &resp)?;
    if refused {
        // Unread request bytes would turn our close into a TCP reset and the
        // client could lose the status line. Half-close, then discard a bounded
        // amount (the read timeout still applies).
        let _ = closer.shutdown(Shutdown::Write);
        let _ = std::io::copy(&mut reader.take(64 * 1024), &mut std::io::sink());
    }
    Ok(())
}

pub fn serve(addr: &str, app: Arc<RwLock<App>>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("sdx: listening on http://{}", listener.local_addr()?);
    static CONNS: AtomicUsize = AtomicUsize::new(0);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if CONNS.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            CONNS.fetch_sub(1, Ordering::AcqRel);
            let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
            let _ = write_response(stream, &simple(503));
            continue;
        }
        let app = Arc::clone(&app);
        std::thread::spawn(move || {
            let _ = handle(stream, &app);
            CONNS.fetch_sub(1, Ordering::AcqRel);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn head_ok() {
        let mut c = Cursor::new(b"GET /order_book?x=1 HTTP/1.1\r\nHost: a\r\n\r\n".to_vec());
        assert_eq!(read_head(&mut c).unwrap().trim(), "GET /order_book?x=1 HTTP/1.1");
    }

    #[test]
    fn long_request_line_is_414() {
        let mut v = b"GET /".to_vec();
        v.extend(std::iter::repeat_n(b'a', 20_000));
        v.extend_from_slice(b" HTTP/1.1\r\n\r\n");
        assert_eq!(read_head(&mut Cursor::new(v)), Err(414));
    }

    #[test]
    fn long_header_line_is_431() {
        let mut v = b"GET / HTTP/1.1\r\nX: ".to_vec();
        v.extend(std::iter::repeat_n(b'a', 20_000));
        v.extend_from_slice(b"\r\n\r\n");
        assert_eq!(read_head(&mut Cursor::new(v)), Err(431));
    }

    #[test]
    fn too_many_headers_is_431() {
        let mut v = b"GET / HTTP/1.1\r\n".to_vec();
        for _ in 0..200 {
            v.extend_from_slice(b"X: y\r\n");
        }
        v.extend_from_slice(b"\r\n");
        assert_eq!(read_head(&mut Cursor::new(v)), Err(431));
    }

    #[test]
    fn empty_is_400() {
        assert_eq!(read_head(&mut Cursor::new(Vec::new())), Err(400));
    }

    #[test]
    fn slot_cap() {
        let c = AtomicUsize::new(0);
        let a = Slot::take(&c, 2);
        let b = Slot::take(&c, 2);
        assert!(a.is_some() && b.is_some());
        assert!(Slot::take(&c, 2).is_none());
        drop(a);
        assert!(Slot::take(&c, 2).is_some());
    }
}
