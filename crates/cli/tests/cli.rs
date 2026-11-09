// SPDX-License-Identifier: Apache-2.0
//! End-to-end tests of the `sdx` binary over a synthetic state directory
//! (labelled synthetic: hand-built entries, not mainnet data).

use sdx_core::snapshot;
use sdx_core::state::{ApplyStats, BookState};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use stellar_xdr::*;

const ISSUER: &str = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";

fn usdc() -> Asset {
    let pk: stellar_strkey::ed25519::PublicKey = ISSUER.parse().unwrap();
    Asset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4(*b"USDC"),
        issuer: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(pk.0))),
    })
}

fn offer(id: i64, selling: Asset, buying: Asset, amount: i64, n: i32, d: i32) -> LedgerEntry {
    LedgerEntry {
        last_modified_ledger_seq: 1,
        data: LedgerEntryData::Offer(OfferEntry {
            seller_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([7; 32]))),
            offer_id: id,
            selling,
            buying,
            amount,
            price: Price { n, d },
            flags: 0,
            ext: OfferEntryExt::V0,
        }),
        ext: LedgerEntryExt::V0,
    }
}

fn state_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sdx-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = BookState::new();
    let mut st = ApplyStats::default();
    // asks: sell XLM for USDC at 0.16 and 0.17; bid: sell USDC for XLM at 6.25 XLM/USDC (= 0.16 USDC/XLM)
    s.upsert(&offer(1, Asset::Native, usdc(), 1000_0000000, 4, 25), &mut st).unwrap();
    s.upsert(&offer(2, Asset::Native, usdc(), 500_0000000, 17, 100), &mut st).unwrap();
    s.upsert(&offer(3, usdc(), Asset::Native, 100_0000000, 13, 2), &mut st).unwrap();
    s.ledger = 100;
    s.close_time = 1_700_000_000;
    snapshot::save(&dir, &s, 63, serde_json::json!({"synthetic": true})).unwrap();
    dir
}

fn sdx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sdx"))
}

#[test]
fn order_book_and_paths_commands() {
    let dir = state_dir("cmd");
    let out = sdx()
        .args(["order-book", "--state"])
        .arg(&dir)
        .args(["--selling", "native", "--buying", &format!("USDC:{ISSUER}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["asks"][0]["price"], "0.1600000");
    assert_eq!(v["asks"][0]["amount"], "1000.0000000");
    assert_eq!(v["asks"][1]["price_r"], serde_json::json!({"n": 17, "d": 100}));
    assert_eq!(v["bids"][0]["price"], "0.1538462");
    assert_eq!(v["base"]["asset_type"], "native");

    let out = sdx()
        .args(["strict-send", "--state"])
        .arg(&dir)
        .args(["--source", "native", "--amount", "100", "--destinations", &format!("USDC:{ISSUER}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    // spend 100 XLM against the bid offer (sells USDC at 6.5 XLM/USDC): floor(100 * 2 / 13) USDC
    assert_eq!(v["_embedded"]["records"][0]["destination_amount"], "15.3846153");

    let bad = sdx().args(["order-book", "--state"]).arg(&dir).args(["--selling", "native", "--buying", "USDC:GBAD"]).output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn http_server_serves_horizon_shapes() {
    let dir = state_dir("http");
    let mut child = sdx().args(["serve", "--state"]).arg(&dir).args(["--addr", "127.0.0.1:0"]).stderr(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stderr.take().unwrap()).read_line(&mut line).unwrap();
    let addr = line.trim().rsplit("http://").next().unwrap().to_string();
    let get = |target: &str| -> (String, String) {
        let mut s = TcpStream::connect(&addr).unwrap();
        write!(s, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut buf = String::new();
        s.read_to_string(&mut buf).unwrap();
        let (head, body) = buf.split_once("\r\n\r\n").unwrap();
        (head.to_string(), body.to_string())
    };
    let (head, body) = get(&format!(
        "/order_book?selling_asset_type=native&buying_asset_type=credit_alphanum4&buying_asset_code=USDC&buying_asset_issuer={ISSUER}&limit=1"
    ));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("Latest-Ledger: 100"));
    assert!(head.contains("Access-Control-Allow-Origin: *"), "{head}");
    assert!(head.contains("Access-Control-Expose-Headers: Latest-Ledger"), "{head}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["asks"].as_array().unwrap().len(), 1);
    let (head, body) = get("/paths/strict-receive?destination_asset_type=native&destination_amount=1&source_account=GA5Z");
    assert!(head.starts_with("HTTP/1.1 400"), "{head}");
    assert!(body.contains("source_account is not supported"));
    let (head, _) = get("/nope");
    assert!(head.starts_with("HTTP/1.1 404"));
    let (_, body) = get("/");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["offers"], 3);
    // SDX-029: an oversized request line is refused, not buffered without bound.
    let (head, _) = get(&format!("/{}", "a".repeat(20_000)));
    assert!(head.starts_with("HTTP/1.1 414"), "{}", &head[..40.min(head.len())]);
    // a client that sends a partial request line and stalls does not wedge the server
    let mut stalled = TcpStream::connect(&addr).unwrap();
    write!(stalled, "GET /").unwrap();
    let (head, _) = get("/nope");
    assert!(head.starts_with("HTTP/1.1 404"));
    drop(stalled);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
