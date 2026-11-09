// SPDX-License-Identifier: Apache-2.0
//! Classic assets as canonical strings.
//!
//! The canonical form is the one go-stellar-sdk's `xdr.Asset.String()` prints:
//! `native`, `credit_alphanum4/USDC/GA5Z...`, `credit_alphanum12/LONGCODE/G...`.
//! Horizon sorts path results by this string, so we use it as the asset key.

use crate::error::{Error, Result};
use serde_json::{Map, Value};
use stellar_xdr::{AccountId, Asset, PublicKey};

pub const NATIVE: &str = "native";

pub fn account_str(a: &AccountId) -> String {
    let PublicKey::PublicKeyTypeEd25519(k) = &a.0;
    stellar_strkey::ed25519::PublicKey(k.0).to_string()
}

pub fn account_bytes(a: &AccountId) -> [u8; 32] {
    let PublicKey::PublicKeyTypeEd25519(k) = &a.0;
    k.0
}

fn code_str(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Canonical string of an XDR asset.
pub fn from_xdr(a: &Asset) -> String {
    match a {
        Asset::Native => NATIVE.to_string(),
        Asset::CreditAlphanum4(x) => format!("credit_alphanum4/{}/{}", code_str(&x.asset_code.0), account_str(&x.issuer)),
        Asset::CreditAlphanum12(x) => format!("credit_alphanum12/{}/{}", code_str(&x.asset_code.0), account_str(&x.issuer)),
    }
}

fn valid_code(code: &str) -> bool {
    !code.is_empty() && code.len() <= 12 && code.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn valid_issuer(issuer: &str) -> bool {
    issuer.parse::<stellar_strkey::ed25519::PublicKey>().is_ok()
}

/// Build from Horizon's `*_asset_type`, `*_asset_code`, `*_asset_issuer` query params.
pub fn from_parts(asset_type: &str, code: &str, issuer: &str) -> Result<String> {
    match asset_type {
        "native" => {
            if !code.is_empty() || !issuer.is_empty() {
                return Err(Error::bad("native asset takes no code or issuer"));
            }
            Ok(NATIVE.to_string())
        }
        "credit_alphanum4" | "credit_alphanum12" => {
            let want4 = asset_type == "credit_alphanum4";
            if !valid_code(code) || (want4 && code.len() > 4) || (!want4 && code.len() < 5) {
                return Err(Error::bad(format!("invalid asset code {code:?} for {asset_type}")));
            }
            if !valid_issuer(issuer) {
                return Err(Error::bad(format!("invalid asset issuer {issuer:?}")));
            }
            Ok(format!("{asset_type}/{code}/{issuer}"))
        }
        other => Err(Error::bad(format!("invalid asset type {other:?}"))),
    }
}

/// Parse one entry of Horizon's comma-separated asset lists (`native` or `CODE:ISSUER`),
/// also accepted by the CLI.
pub fn parse_short(s: &str) -> Result<String> {
    let s = s.trim();
    if s == NATIVE {
        return Ok(NATIVE.to_string());
    }
    let (code, issuer) = s.split_once(':').ok_or_else(|| Error::bad(format!("invalid asset {s:?}: want native or CODE:ISSUER")))?;
    let t = if code.len() <= 4 { "credit_alphanum4" } else { "credit_alphanum12" };
    from_parts(t, code, issuer)
}

pub fn parse_list(s: &str) -> Result<Vec<String>> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    s.split(',').map(parse_short).collect()
}

/// `native` or `CODE:ISSUER`.
pub fn short(canonical: &str) -> String {
    match split(canonical) {
        None => NATIVE.to_string(),
        Some((_, code, issuer)) => format!("{code}:{issuer}"),
    }
}

/// (type, code, issuer) for credit assets, `None` for native.
pub fn split(canonical: &str) -> Option<(&str, &str, &str)> {
    let mut it = canonical.splitn(3, '/');
    let t = it.next()?;
    let code = it.next()?;
    let issuer = it.next()?;
    Some((t, code, issuer))
}

/// Horizon JSON fields for an asset, with an optional prefix (`source_`, ...).
pub fn json_fields(canonical: &str, prefix: &str, out: &mut Map<String, Value>) {
    match split(canonical) {
        None => {
            out.insert(format!("{prefix}asset_type"), Value::from(NATIVE));
        }
        Some((t, code, issuer)) => {
            out.insert(format!("{prefix}asset_type"), Value::from(t));
            out.insert(format!("{prefix}asset_code"), Value::from(code));
            out.insert(format!("{prefix}asset_issuer"), Value::from(issuer));
        }
    }
}

pub fn json(canonical: &str) -> Value {
    let mut m = Map::new();
    json_fields(canonical, "", &mut m);
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const USDC_ISSUER: &str = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";

    #[test]
    fn canonical_strings() {
        let usdc = parse_short(&format!("USDC:{USDC_ISSUER}")).unwrap();
        assert_eq!(usdc, format!("credit_alphanum4/USDC/{USDC_ISSUER}"));
        assert_eq!(short(&usdc), format!("USDC:{USDC_ISSUER}"));
        assert_eq!(parse_short("native").unwrap(), "native");
        let long = parse_short(&format!("LONGCODE:{USDC_ISSUER}")).unwrap();
        assert!(long.starts_with("credit_alphanum12/LONGCODE/"));
        assert!(parse_short("USDC:GBAD").is_err());
        assert!(from_parts("credit_alphanum4", "TOOLONG", USDC_ISSUER).is_err());
        assert!(from_parts("credit_alphanum12", "ABC", USDC_ISSUER).is_err());
        assert!(from_parts("native", "XLM", "").is_err());
    }

    #[test]
    fn xdr_round_trip() {
        let pk: stellar_strkey::ed25519::PublicKey = USDC_ISSUER.parse().unwrap();
        let a = Asset::CreditAlphanum4(stellar_xdr::AlphaNum4 {
            asset_code: stellar_xdr::AssetCode4(*b"USD\0"),
            issuer: AccountId(PublicKey::PublicKeyTypeEd25519(stellar_xdr::Uint256(pk.0))),
        });
        assert_eq!(from_xdr(&a), format!("credit_alphanum4/USD/{USDC_ISSUER}"));
        assert_eq!(from_xdr(&Asset::Native), "native");
    }

    #[test]
    fn json_shape() {
        let v = json(&format!("credit_alphanum4/USDC/{USDC_ISSUER}"));
        assert_eq!(v["asset_type"], "credit_alphanum4");
        assert_eq!(v["asset_code"], "USDC");
        assert_eq!(json("native"), serde_json::json!({"asset_type": "native"}));
    }
}
