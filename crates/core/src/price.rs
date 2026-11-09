// SPDX-License-Identifier: Apache-2.0
//! Exact price arithmetic.
//!
//! `mul_fraction_round_down`, `mul_fraction_round_up` and
//! `convert_to_buying_units` are ports of go-stellar-sdk `price/main.go`
//! (Apache-2.0, see NOTICE), which mirror stellar-core's exchange rounding.

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceError {
    Overflow,
    DivisionByZero,
    Negative,
}

/// floor(x * n / d), error if the result does not fit in i64.
pub fn mul_fraction_round_down(x: i64, n: i64, d: i64) -> Result<i64, PriceError> {
    if d == 0 {
        return Err(PriceError::DivisionByZero);
    }
    if x < 0 || n < 0 || d < 0 {
        return Err(PriceError::Negative);
    }
    let q = (x as u128 * n as u128) / d as u128;
    if q > i64::MAX as u128 {
        return Err(PriceError::Overflow);
    }
    Ok(q as i64)
}

/// ceil(x * n / d), error if the result does not fit in i64.
pub fn mul_fraction_round_up(x: i64, n: i64, d: i64) -> Result<i64, PriceError> {
    if d == 0 {
        return Err(PriceError::DivisionByZero);
    }
    if x < 0 || n < 0 || d < 0 {
        return Err(PriceError::Negative);
    }
    let q = (x as u128 * n as u128).div_ceil(d as u128);
    if q > i64::MAX as u128 {
        return Err(PriceError::Overflow);
    }
    Ok(q as i64)
}

/// go-stellar-sdk `price.ConvertToBuyingUnits`: how many buying units an offer
/// with `selling_offer_amount` at price n/d takes for `selling_units_needed`.
/// Returns (buying units paid, selling units extracted).
pub fn convert_to_buying_units(
    selling_offer_amount: i64,
    selling_units_needed: i64,
    pricen: i64,
    priced: i64,
) -> Result<(i64, i64), PriceError> {
    let mut result = selling_offer_amount;
    if pricen <= priced {
        result = mul_fraction_round_down(selling_offer_amount, pricen, priced)?;
        result = mul_fraction_round_up(result, priced, pricen)?;
    }
    result = result.min(selling_units_needed);
    let extracted = result;
    let paid = mul_fraction_round_up(result, pricen, priced)?;
    Ok((paid, extracted))
}

/// Compare n1/d1 with n2/d2 exactly (denominators positive).
pub fn cmp(n1: i64, d1: i64, n2: i64, d2: i64) -> Ordering {
    (n1 as i128 * d2 as i128).cmp(&(n2 as i128 * d1 as i128))
}

pub fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// Lowest terms (big.Rat normalisation).
pub fn reduce(n: i128, d: i128) -> (i128, i128) {
    let g = gcd(n, d);
    if g == 0 {
        return (n, d);
    }
    let (mut n, mut d) = (n / g, d / g);
    if d < 0 {
        n = -n;
        d = -d;
    }
    (n, d)
}

/// Go `big.Rat.FloatString(7)`: 7 decimals, last digit rounded to nearest,
/// halves away from zero.
pub fn float_string7(n: i128, d: i128) -> String {
    assert!(d != 0, "float_string7: zero denominator");
    let neg = (n < 0) != (d < 0) && n != 0;
    let (n, d) = (n.unsigned_abs(), d.unsigned_abs());
    let scaled = n * 10_000_000;
    let mut q = scaled / d;
    let r = scaled % d;
    if 2 * r >= d {
        q += 1;
    }
    let s = format!("{}.{:07}", q / 10_000_000, q % 10_000_000);
    if neg && q != 0 {
        format!("-{s}")
    } else {
        s
    }
}

/// Decimal string of n/d with `digits` fractional digits, truncated. Used to
/// emulate Postgres numeric division before a float64 parse.
pub fn decimal_div(n: i128, d: i128, digits: u32) -> String {
    let (n, d) = (n.unsigned_abs(), d.unsigned_abs());
    let int = n / d;
    let mut r = n % d;
    let mut s = format!("{int}.");
    for _ in 0..digits {
        r *= 10;
        s.push(char::from(b'0' + (r / d) as u8));
        r %= d;
    }
    s
}

/// Horizon `avg`: Postgres `sum(counter)::numeric / sum(base)::numeric`, scanned
/// into float64 and printed with `strconv.FormatFloat(v, 'f', 7, 64)`.
pub fn pg_avg_string(counter: i128, base: i128) -> String {
    if base == 0 {
        return "0.0000000".to_string();
    }
    // 30 fractional digits is far more than Postgres keeps (>= 16 significant);
    // the float64 parse makes the extra digits irrelevant except at exact ties.
    let v: f64 = decimal_div(counter, base, 30).parse().unwrap_or(0.0);
    format!("{v:.7}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_fraction() {
        assert_eq!(mul_fraction_round_down(10, 1, 3), Ok(3));
        assert_eq!(mul_fraction_round_up(10, 1, 3), Ok(4));
        assert_eq!(mul_fraction_round_up(9, 1, 3), Ok(3));
        assert_eq!(mul_fraction_round_down(i64::MAX, 2, 1), Err(PriceError::Overflow));
        assert_eq!(mul_fraction_round_down(1, 1, 0), Err(PriceError::DivisionByZero));
        assert_eq!(mul_fraction_round_down(-1, 1, 1), Err(PriceError::Negative));
    }

    #[test]
    fn convert_units_matches_go_vectors() {
        // Vectors from go-stellar-sdk price/main_test.go TestConvertToBuyingUnits
        // (re-derived by hand: offer 1000 @ 1/2, need 500 -> pay ceil(500/2)=250).
        assert_eq!(convert_to_buying_units(1000, 500, 1, 2), Ok((250, 500)));
        // Offer smaller than need: extract all 1000 @ 2/1 -> pay 2000.
        assert_eq!(convert_to_buying_units(1000, 5000, 2, 1), Ok((2000, 1000)));
        // pricen <= priced rounds the offer down to an exchangeable amount: 7 @ 2/3
        // -> floor(7*2/3)=4 -> ceil(4*3/2)=6 extractable, pay ceil(6*2/3)=4.
        assert_eq!(convert_to_buying_units(7, 100, 2, 3), Ok((4, 6)));
    }

    #[test]
    fn float_strings() {
        assert_eq!(float_string7(25397, 156249), "0.1625418");
        assert_eq!(float_string7(1, 3), "0.3333333");
        assert_eq!(float_string7(2, 3), "0.6666667");
        // half away from zero at the 8th digit
        assert_eq!(float_string7(5, 100_000_000), "0.0000001");
        assert_eq!(float_string7(-5, 100_000_000), "-0.0000001");
        assert_eq!(float_string7(156249, 25397), "6.1522621");
    }

    #[test]
    fn reduce_and_cmp() {
        assert_eq!(reduce(1_627_605, 10_000_000), (325_521, 2_000_000));
        assert_eq!(cmp(1, 2, 2, 4), Ordering::Equal);
        assert_eq!(cmp(1, 3, 1, 2), Ordering::Less);
    }

    #[test]
    fn avg_like_horizon() {
        // Horizon record captured 2026-11-03: base 1863.4584378, counter 303.3234087, avg 0.1627744
        assert_eq!(pg_avg_string(3_033_234_087, 18_634_584_378), "0.1627744");
        assert_eq!(pg_avg_string(275_154_102, 1_692_553_629), "0.1625674");
    }
}
