// SPDX-License-Identifier: Apache-2.0
//! Stroop amounts in Horizon's decimal string form ("12.3456789").

use crate::error::{Error, Result};

pub const ONE: i64 = 10_000_000;

/// Horizon `amount.String` / `amount.IntStringToAmount`: always 7 decimals.
pub fn format(stroops: i128) -> String {
    let neg = stroops < 0;
    let v = stroops.unsigned_abs();
    let s = format!("{}.{:07}", v / ONE as u128, v % ONE as u128);
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

/// Horizon `amount.Parse`: positive decimal with at most 7 fractional digits,
/// result must fit in i64. Rejects signs, exponents and empty parts.
pub fn parse(s: &str) -> Result<i64> {
    let bad = || Error::bad(format!("invalid amount {s:?}"));
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int.is_empty() && frac.is_empty() {
        return Err(bad());
    }
    if !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) || frac.len() > 7 {
        return Err(bad());
    }
    let i: i128 = if int.is_empty() { 0 } else { int.parse().map_err(|_| bad())? };
    let mut f: i128 = if frac.is_empty() { 0 } else { frac.parse().map_err(|_| bad())? };
    for _ in frac.len()..7 {
        f *= 10;
    }
    let v = i.checked_mul(ONE as i128).and_then(|x| x.checked_add(f)).ok_or_else(bad)?;
    if v > i64::MAX as i128 {
        return Err(bad());
    }
    Ok(v as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_horizon() {
        assert_eq!(format(0), "0.0000000");
        assert_eq!(format(1), "0.0000001");
        assert_eq!(format(1_000_000_000), "100.0000000");
        assert_eq!(format(42_274_963), "4.2274963");
        assert_eq!(format(-5), "-0.0000005");
        assert_eq!(format(i64::MAX as i128), "922337203685.4775807");
    }

    #[test]
    fn parses_amounts() {
        assert_eq!(parse("100").unwrap(), 1_000_000_000);
        assert_eq!(parse("0.0000001").unwrap(), 1);
        assert_eq!(parse("16.2521166").unwrap(), 162_521_166);
        assert_eq!(parse(".5").unwrap(), 5_000_000);
        assert!(parse("1.12345678").is_err());
        assert!(parse("-1").is_err());
        assert!(parse("1e5").is_err());
        assert!(parse("").is_err());
        assert!(parse("922337203685.4775808").is_err());
    }
}
