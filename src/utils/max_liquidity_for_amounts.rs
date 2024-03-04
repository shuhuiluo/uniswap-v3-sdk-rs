use super::u256_to_big_uint;
use alloy_primitives::U256;
use num_bigint::BigUint;

/// Returns an imprecise maximum amount of liquidity received for a given amount of token 0.
/// This function is available to accommodate LiquidityAmounts#getLiquidityForAmount0 in the v3
/// periphery, which could be more precise by at least 32 bits by dividing by Q64 instead of Q96 in
/// the intermediate step, and shifting the subtracted ratio left by 32 bits. This imprecise
/// calculation will likely be replaced in a future v3 router contract.
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: The price at the lower boundary
/// * `sqrt_ratio_b_x96`: The price at the upper boundary
/// * `amount0`: The token0 amount
///
/// returns: liquidity for amount0, imprecise
pub fn max_liquidity_for_amount0_imprecise(
    mut sqrt_ratio_a_x96: U256,
    mut sqrt_ratio_b_x96: U256,
    amount0: U256,
) -> BigUint {
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = (sqrt_ratio_b_x96, sqrt_ratio_a_x96);
    }
    let sqrt_ratio_a_x96 = u256_to_big_uint(sqrt_ratio_a_x96);
    let sqrt_ratio_b_x96 = u256_to_big_uint(sqrt_ratio_b_x96);

    let intermediate = (&sqrt_ratio_a_x96 * &sqrt_ratio_b_x96) >> 96;
    u256_to_big_uint(amount0) * intermediate / (sqrt_ratio_b_x96 - sqrt_ratio_a_x96)
}

/// Returns a precise maximum amount of liquidity received for a given amount of token 0 by dividing
/// by Q64 instead of Q96 in the intermediate step, and shifting the subtracted ratio left by 32
/// bits.
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: The price at the lower boundary
/// * `sqrt_ratio_b_x96`: The price at the upper boundary
/// * `amount0`: The token0 amount
///
/// returns: liquidity for amount0, precise
pub fn max_liquidity_for_amount0_precise(
    mut sqrt_ratio_a_x96: U256,
    mut sqrt_ratio_b_x96: U256,
    amount0: U256,
) -> BigUint {
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = (sqrt_ratio_b_x96, sqrt_ratio_a_x96);
    }
    let sqrt_ratio_a_x96 = u256_to_big_uint(sqrt_ratio_a_x96);
    let sqrt_ratio_b_x96 = u256_to_big_uint(sqrt_ratio_b_x96);

    let numerator = u256_to_big_uint(amount0) * &sqrt_ratio_a_x96 * &sqrt_ratio_b_x96;
    let denominator = (sqrt_ratio_b_x96 - sqrt_ratio_a_x96) << 96;

    numerator / denominator
}

/// Computes the maximum amount of liquidity received for a given amount of token1
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: The price at the lower boundary
/// * `sqrt_ratio_b_x96`: The price at the upper boundary
/// * `amount1`: The token1 amount
///
/// returns: liquidity for amount1
pub fn max_liquidity_for_amount1(
    mut sqrt_ratio_a_x96: U256,
    mut sqrt_ratio_b_x96: U256,
    amount1: U256,
) -> BigUint {
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = (sqrt_ratio_b_x96, sqrt_ratio_a_x96);
    }
    let sqrt_ratio_a_x96 = u256_to_big_uint(sqrt_ratio_a_x96);
    let sqrt_ratio_b_x96 = u256_to_big_uint(sqrt_ratio_b_x96);

    (u256_to_big_uint(amount1) << 96) / (sqrt_ratio_b_x96 - sqrt_ratio_a_x96)
}

/// Computes the maximum amount of liquidity received for a given amount of token0, token1,
/// and the prices at the tick boundaries.
///
/// ## Arguments
///
/// * `sqrt_ratio_current_x96`: The current price
/// * `sqrt_ratio_a_x96`: The price at the lower boundary
/// * `sqrt_ratio_b_x96`: The price at the upper boundary
/// * `amount0`: The token0 amount
/// * `amount1`: The token1 amount
/// * `use_full_precision`: if false, liquidity will be maximized according to what the router can
///   calculate, not what core can theoretically support
///
/// returns: maximum liquidity for the given amounts
pub fn max_liquidity_for_amounts(
    sqrt_ratio_current_x96: U256,
    mut sqrt_ratio_a_x96: U256,
    mut sqrt_ratio_b_x96: U256,
    amount0: U256,
    amount1: U256,
    use_full_precision: bool,
) -> BigUint {
    if sqrt_ratio_a_x96 > sqrt_ratio_b_x96 {
        (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = (sqrt_ratio_b_x96, sqrt_ratio_a_x96);
    }

    if sqrt_ratio_current_x96 <= sqrt_ratio_a_x96 {
        if use_full_precision {
            max_liquidity_for_amount0_precise(sqrt_ratio_a_x96, sqrt_ratio_b_x96, amount0)
        } else {
            max_liquidity_for_amount0_imprecise(sqrt_ratio_a_x96, sqrt_ratio_b_x96, amount0)
        }
    } else if sqrt_ratio_current_x96 < sqrt_ratio_b_x96 {
        let liquidity0 = if use_full_precision {
            max_liquidity_for_amount0_precise(sqrt_ratio_current_x96, sqrt_ratio_b_x96, amount0)
        } else {
            max_liquidity_for_amount0_imprecise(sqrt_ratio_current_x96, sqrt_ratio_b_x96, amount0)
        };
        let liquidity1 =
            max_liquidity_for_amount1(sqrt_ratio_a_x96, sqrt_ratio_current_x96, amount1);

        if liquidity0 < liquidity1 {
            liquidity0
        } else {
            liquidity1
        }
    } else {
        max_liquidity_for_amount1(sqrt_ratio_a_x96, sqrt_ratio_b_x96, amount1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::encode_sqrt_ratio_x96;
    use num_traits::Num;

    #[test]
    fn imprecise_price_inside_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(1, 1),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                false
            ),
            2148u64.into()
        );
    }

    #[test]
    fn imprecise_price_inside_100_token0_max_token1() {
        let res = max_liquidity_for_amounts(
            encode_sqrt_ratio_x96(1, 1),
            encode_sqrt_ratio_x96(100, 110),
            encode_sqrt_ratio_x96(110, 100),
            U256::from(100),
            U256::MAX,
            false,
        );
        assert_eq!(res, 2148u64.into());
    }

    #[test]
    fn imprecise_price_inside_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(1, 1),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                false
            ),
            4297u64.into()
        );
    }

    #[test]
    fn imprecise_price_below_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                false
            ),
            1048u64.into()
        );
    }

    #[test]
    fn imprecise_price_below_100_token0_max_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::MAX,
                false
            ),
            1048u64.into()
        );
    }

    #[test]
    fn imprecise_price_below_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                false
            ),
            BigUint::from_str_radix(
                "1214437677402050006470401421068302637228917309992228326090730924516431320489727",
                10
            )
            .unwrap()
        );
    }

    #[test]
    fn imprecise_price_above_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                false
            ),
            2097u64.into()
        );
    }

    #[test]
    fn imprecise_price_above_100_token0_max_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::MAX,
                false
            ),
            BigUint::from_str_radix(
                "1214437677402050006470401421098959354205873606971497132040612572422243086574654",
                10
            )
            .unwrap()
        );
    }

    #[test]
    fn imprecise_price_above_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                false
            ),
            2097u64.into()
        );
    }

    #[test]
    fn precise_price_inside_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(1, 1),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                true
            ),
            2148u64.into()
        );
    }

    #[test]
    fn precise_price_inside_100_token0_max_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(1, 1),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::MAX,
                true
            ),
            2148u64.into()
        );
    }

    #[test]
    fn precise_price_inside_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(1, 1),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                true
            ),
            4297u64.into()
        );
    }

    #[test]
    fn precise_price_below_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                true
            ),
            1048u64.into()
        );
    }

    #[test]
    fn precise_price_below_100_token0_max_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::MAX,
                true
            ),
            1048u64.into()
        );
    }

    #[test]
    fn precise_price_below_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(99, 110),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                true
            ),
            BigUint::from_str_radix(
                "1214437677402050006470401421082903520362793114274352355276488318240158678126184",
                10
            )
            .unwrap()
        );
    }

    #[test]
    fn precise_price_above_100_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::from(200),
                true
            ),
            2097u64.into()
        );
    }

    #[test]
    fn precise_price_above_100_token0_max_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::from(100),
                U256::MAX,
                true
            ),
            BigUint::from_str_radix(
                "1214437677402050006470401421098959354205873606971497132040612572422243086574654",
                10
            )
            .unwrap()
        );
    }

    #[test]
    fn precise_price_above_max_token0_200_token1() {
        assert_eq!(
            max_liquidity_for_amounts(
                encode_sqrt_ratio_x96(111, 100),
                encode_sqrt_ratio_x96(100, 110),
                encode_sqrt_ratio_x96(110, 100),
                U256::MAX,
                U256::from(200),
                true
            ),
            2097u64.into()
        );
    }
}
