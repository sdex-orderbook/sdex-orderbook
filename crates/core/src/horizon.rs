// SPDX-License-Identifier: Apache-2.0
//! Horizon-compatible request parsing and response shapes for `/order_book`,
//! `/paths/strict-send`, `/paths/strict-receive` and `/trade_aggregations`.
//! Shared by the HTTP server and the CLI so both print identical JSON.

use crate::aggregate::{self, Minutes};
use crate::amount;
use crate::asset;
use crate::book;
use crate::error::{Code, Error, Result};
use crate::paths::{self, Options};
use crate::state::BookState;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type Params = BTreeMap<String, String>;

fn hexval(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match (hexval(b[i + 1]), hexval(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_query(qs: &str) -> Params {
    let mut m = Params::new();
    for kv in qs.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        m.insert(url_decode(k), url_decode(v));
    }
    m
}

fn get<'a>(p: &'a Params, k: &str) -> &'a str {
    p.get(k).map(String::as_str).unwrap_or("")
}

fn asset_param(p: &Params, prefix: &str) -> Result<String> {
    let t = get(p, &format!("{prefix}asset_type"));
    if t.is_empty() {
        return Err(Error::bad(format!("{prefix}asset_type is required")));
    }
    asset::from_parts(t, get(p, &format!("{prefix}asset_code")), get(p, &format!("{prefix}asset_issuer")))
}

fn limit_param(p: &Params, default: usize, max: usize) -> Result<usize> {
    match p.get("limit") {
        None => Ok(default),
        Some(s) if s.is_empty() => Ok(default),
        Some(s) => {
            let v: usize = s.parse().map_err(|_| Error::bad("limit must be a positive integer"))?;
            if v == 0 || v > max {
                return Err(Error::bad(format!("limit must be between 1 and {max}")));
            }
            Ok(v)
        }
    }
}

/// Horizon problem document.
pub fn problem(e: &Error) -> Value {
    let (t, title) = match e.code {
        Code::BadRequest => ("bad_request", "Bad Request"),
        Code::NotFound => ("not_found", "Resource Missing"),
        _ => ("server_error", "Internal Server Error"),
    };
    json!({"type": t, "title": title, "status": e.code.http_status(), "detail": e.message, "extras": {"code": e.code.as_str()}})
}

/// `GET /order_book`.
pub fn order_book(state: &BookState, p: &Params) -> Result<Value> {
    let base = asset_param(p, "selling_").map_err(|e| Error::bad(format!("invalid_order_book: {}", e.message)))?;
    let counter = asset_param(p, "buying_").map_err(|e| Error::bad(format!("invalid_order_book: {}", e.message)))?;
    let limit = limit_param(p, 20, 200)?;
    let s = book::summary(state, &base, &counter, limit);
    Ok(json!({
        "bids": s.bids.iter().map(book::Level::to_json).collect::<Vec<_>>(),
        "asks": s.asks.iter().map(book::Level::to_json).collect::<Vec<_>>(),
        "base": asset::json(&base),
        "counter": asset::json(&counter),
    }))
}

fn path_options(p: &Params, max_path_length: usize, include_pools: bool) -> Result<Options> {
    let mut o = Options { max_path_length, include_pools, ..Options::default() };
    // Non-Horizon extension (SPEC 5.4): per-request hop limit, capped at the server's.
    if let Some(v) = p.get("max_path_length").filter(|v| !v.is_empty()) {
        let n: usize = v.parse().map_err(|_| Error::bad("max_path_length must be an integer"))?;
        if n == 0 || n > max_path_length {
            return Err(Error::bad(format!("max_path_length must be between 1 and {max_path_length}")));
        }
        o.max_path_length = n;
    }
    Ok(o)
}

fn paths_page(records: &[paths::Path]) -> Value {
    json!({"_embedded": {"records": records.iter().map(paths::Path::to_json).collect::<Vec<_>>()}})
}

/// Horizon `--max-assets-per-path-request` default.
pub const MAX_ASSETS_PARAM: usize = 15;

/// `GET /paths/strict-send`.
pub fn strict_send(state: &BookState, p: &Params, max_path_length: usize, include_pools: bool) -> Result<Value> {
    let dest_account = get(p, "destination_account");
    let dest_assets = get(p, "destination_assets");
    if dest_account.is_empty() == dest_assets.is_empty() {
        return Err(Error::bad(
            "The request requires either a list of destination assets or a destination account. Both fields cannot be present.",
        ));
    }
    if !dest_account.is_empty() {
        return Err(Error::bad(
            "destination_account is not supported: this service tracks no account or trustline state; pass destination_assets",
        ));
    }
    let dests = asset::parse_list(dest_assets)?;
    if dests.len() > MAX_ASSETS_PARAM {
        return Err(Error::bad(format!("list of assets exceeds maximum length of {MAX_ASSETS_PARAM}")));
    }
    let source = asset_param(p, "source_")?;
    let amount = amount::parse(get(p, "source_amount"))?;
    let o = path_options(p, max_path_length, include_pools)?;
    Ok(paths_page(&paths::strict_send(state, &source, amount, &dests, o)?))
}

/// `GET /paths/strict-receive` (and the legacy `/paths`).
pub fn strict_receive(state: &BookState, p: &Params, max_path_length: usize, include_pools: bool) -> Result<Value> {
    let source_account = get(p, "source_account");
    let source_assets = get(p, "source_assets");
    if source_account.is_empty() == source_assets.is_empty() {
        return Err(Error::bad("The request requires either a list of source assets or a source account. Both fields cannot be present."));
    }
    if !source_account.is_empty() {
        return Err(Error::bad("source_account is not supported: this service tracks no account or trustline state; pass source_assets"));
    }
    let sources = asset::parse_list(source_assets)?;
    if sources.len() > MAX_ASSETS_PARAM {
        return Err(Error::bad(format!("list of assets exceeds maximum length of {MAX_ASSETS_PARAM}")));
    }
    let dest = asset_param(p, "destination_")?;
    let amount = amount::parse(get(p, "destination_amount"))?;
    let o = path_options(p, max_path_length, include_pools)?;
    let src: Vec<(String, i64)> = sources.into_iter().map(|a| (a, 0)).collect();
    Ok(paths_page(&paths::strict_receive(state, &dest, amount, &src, false, None, o)?))
}

fn opt_i64(p: &Params, k: &str) -> Result<Option<i64>> {
    match p.get(k).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => v.parse::<i64>().map(Some).map_err(|_| Error::bad(format!("{k} must be an integer"))),
    }
}

/// `GET /trade_aggregations` over trades derived from the replayed ledgers.
/// `self_url` is echoed in `_links.self` (Horizon builds next/prev links too;
/// this service returns `self` only, SPEC 5.5).
pub fn trade_aggregations(minutes: &Minutes, p: &Params, self_url: &str) -> Result<Value> {
    let base = asset_param(p, "base_")?;
    let counter = asset_param(p, "counter_")?;
    let resolution = opt_i64(p, "resolution")?.ok_or_else(|| Error::bad("resolution is required"))?;
    let q = aggregate::Query {
        resolution,
        offset: opt_i64(p, "offset")?.unwrap_or(0),
        start_time: opt_i64(p, "start_time")?,
        end_time: opt_i64(p, "end_time")?,
        descending: match get(p, "order") {
            "" | "asc" => false,
            "desc" => true,
            _ => return Err(Error::bad("order must be asc or desc")),
        },
        limit: limit_param(p, 10, 200)?,
    };
    let rows = aggregate::aggregate(&minutes.oriented(&base, &counter), &q)?;
    Ok(json!({
        "_links": {"self": {"href": self_url}},
        "_embedded": {"records": rows.iter().map(aggregate::Row::to_json).collect::<Vec<_>>()},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parsing() {
        let p = parse_query("a=1&b=x%3Ay&c=&d+e=f+g&bad=%zz&tail=%4");
        assert_eq!(p["a"], "1");
        assert_eq!(p["b"], "x:y");
        assert_eq!(p["c"], "");
        assert_eq!(p["d e"], "f g");
        assert_eq!(p["bad"], "%zz");
        assert_eq!(p["tail"], "%4");
    }

    #[test]
    fn param_errors_are_400() {
        let s = BookState::new();
        let e = order_book(&s, &parse_query("selling_asset_type=native")).unwrap_err();
        assert_eq!(e.code.http_status(), 400);
        let e = strict_receive(&s, &parse_query("source_account=G&source_assets=native"), 3, true).unwrap_err();
        assert_eq!(e.code, Code::BadRequest);
        let e =
            strict_send(&s, &parse_query("destination_assets=native&source_asset_type=native&source_amount=1&max_path_length=9"), 3, true)
                .unwrap_err();
        assert_eq!(e.code, Code::BadRequest);
        let ok = order_book(&s, &parse_query("selling_asset_type=native&buying_asset_type=native")).unwrap();
        assert_eq!(ok["bids"], json!([]));
        assert_eq!(problem(&e)["status"], 400);
    }
}
