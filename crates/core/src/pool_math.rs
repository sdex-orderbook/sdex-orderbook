// SPDX-License-Identifier: Apache-2.0
//! Constant-product pool exchange maths.
//!
//! Port of go-stellar-sdk `exp/orderbook/pools.go` `CalculatePoolPayout` and
//! `CalculatePoolExpectation` (Apache-2.0, see NOTICE), including the
//! rounding-slippage figure Horizon uses to filter trade aggregations.

use ethnum::U256;

pub const MAX_BASIS_POINTS: i64 = 10_000;

/// Signed |a - b| on U256 values (Go's `uint256.Sub` + `Abs` on two's complement).
fn abs_diff(a: U256, b: U256) -> U256 {
    if a >= b {
        a - b
    } else {
        b - a
    }
}

fn slippage(numer: U256, denom: U256, result: U256) -> Option<i64> {
    let max_bips = U256::from(MAX_BASIS_POINTS as u64);
    let unrounded = numer * max_bips / denom;
    let rounded = result * max_bips;
    if unrounded == U256::ZERO {
        // holiman/uint256 defines x / 0 = 0.
        return Some(0);
    }
    let s = abs_diff(unrounded, rounded) * max_bips / unrounded / U256::from(100u64);
    if s > U256::from(u64::MAX) {
        return None;
    }
    let v = s.as_u64() as i64;
    (v >= 0).then_some(v)
}

/// Deposit `received` of asset A into a pool with reserves (A, B); how much B
/// comes out. Returns (payout, rounding slippage bips) or `None` on overflow.
pub fn calculate_pool_payout(reserve_a: i64, reserve_b: i64, received: i64, fee_bips: i32, with_slippage: bool) -> Option<(i64, i64)> {
    if reserve_a < 0 || reserve_b < 0 || received < 0 {
        return None;
    }
    if fee_bips < 0 || fee_bips as i64 >= MAX_BASIS_POINTS {
        return None;
    }
    if received > i64::MAX - reserve_a {
        return None;
    }
    let x_res = U256::from(reserve_a as u64);
    let y_res = U256::from(reserve_b as u64);
    let x = U256::from(received as u64);
    let max_bips = U256::from(MAX_BASIS_POINTS as u64);
    let f = max_bips - U256::from(fee_bips as u64);
    let denom = x_res * max_bips + x * f;
    if denom == U256::ZERO {
        return None;
    }
    let numer = y_res * x * f;
    let result = numer / denom;
    let mut slip = 0;
    if with_slippage && numer % denom != U256::ZERO {
        slip = slippage(numer, denom, result)?;
    }
    if result > U256::from(u64::MAX) {
        return None;
    }
    let val = result.as_u64() as i64;
    (val > 0).then_some((val, slip))
}

/// Withdraw `disbursed` of asset B from a pool with reserves (A, B); how much A
/// must go in. Returns (deposit, rounding slippage bips) or `None`.
pub fn calculate_pool_expectation(
    reserve_a: i64,
    reserve_b: i64,
    disbursed: i64,
    fee_bips: i32,
    with_slippage: bool,
) -> Option<(i64, i64)> {
    if reserve_a < 0 || reserve_b < 0 || disbursed < 0 {
        return None;
    }
    if fee_bips < 0 || fee_bips as i64 >= MAX_BASIS_POINTS {
        return None;
    }
    if disbursed >= reserve_b {
        return None;
    }
    let x_res = U256::from(reserve_a as u64);
    let y_res = U256::from(reserve_b as u64);
    let y = U256::from(disbursed as u64);
    let max_bips = U256::from(MAX_BASIS_POINTS as u64);
    let f = max_bips - U256::from(fee_bips as u64);
    let denom = (y_res - y) * f;
    if denom == U256::ZERO {
        return None;
    }
    let numer = x_res * y * max_bips;
    let mut result = numer / denom;
    let rem = numer % denom;
    let mut slip = 0;
    if rem != U256::ZERO {
        result += U256::ONE;
        if with_slippage {
            slip = slippage(numer, denom, result)?;
        }
    }
    if result > U256::from(u64::MAX) {
        return None;
    }
    let val = result.as_u64() as i64;
    if val < 0 || val > i64::MAX - reserve_a {
        return None;
    }
    Some((val, slip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_basic() {
        // 1000/1000 pool, 30 bps, deposit 100: floor(1000*100*9970 / (1000*10000 + 100*9970)) = 90
        assert_eq!(calculate_pool_payout(1000, 1000, 100, 30, false), Some((90, 0)));
        assert_eq!(calculate_pool_payout(0, 0, 0, 30, false), None);
        assert_eq!(calculate_pool_payout(1000, 1000, 100, 10_000, false), None);
    }

    #[test]
    fn expectation_basic() {
        // withdraw 90 from 1000/1000: ceil(1000*90*10000 / (910*9970)) = 100 (99.19 rounded up)
        assert_eq!(calculate_pool_expectation(1000, 1000, 90, 30, false), Some((100, 0)));
        assert_eq!(calculate_pool_expectation(1000, 1000, 1000, 30, false), None);
    }

    #[test]
    fn payout_and_expectation_are_consistent() {
        for &(a, b, x) in &[(5_000_000_000i64, 3_000_000_000i64, 12_345_678i64), (10i64.pow(15), 7 * 10i64.pow(14), 99_999_999)] {
            let (out, _) = calculate_pool_payout(a, b, x, 30, false).unwrap();
            let (back, _) = calculate_pool_expectation(a, b, out, 30, false).unwrap();
            assert!(back <= x, "expectation {back} must not exceed deposit {x}");
        }
    }

    #[test]
    fn payout_matches_horizon_probe_2026_11_08() {
        // fixtures/probes/20261108-pool-18-usdc: pool 05442dce... reserves 0.0129024 "18" /
        // 13.0498007 USDC, fee 30 bps; Horizon /paths/strict-send for 0.0010000 "18" returned
        // the direct (path []) record destination_amount 0.9360584 (Latest-Ledger 65,365,369).
        assert_eq!(calculate_pool_payout(129_024, 130_498_007, 10_000, 30, false), Some((9_360_584, 0)));
    }

    #[test]
    fn slippage_is_reported() {
        let (_, s) = calculate_pool_payout(1000, 1000, 3, 30, true).unwrap();
        assert!(s > 0);
        let (_, s0) = calculate_pool_payout(1000, 1000, 100, 30, true).unwrap();
        assert!(s0 < 1000);
    }
}
