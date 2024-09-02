//! ## Sqrt Price Math Library in Rust
//! This library is a Rust port of the [SqrtPriceMath library](https://github.com/uniswap/v3-core/blob/main/contracts/libraries/SqrtPriceMath.sol) in Solidity,
//! with custom optimizations presented in [uni-v3-lib](https://github.com/Aperture-Finance/uni-v3-lib/blob/main/src/SqrtPriceMath.sol).

use crate::prelude::*;
use alloy_primitives::{ruint::UintTryFrom, I256, U160, U256};
use num_traits::Zero;

const MAX_U160: U256 = U256::from_limbs([u64::MAX, u64::MAX, u32::MAX as u64, 0]);

/// Gets the next sqrt price given a delta of token0
///
/// Always rounds up, because in the exact output case (increasing price) we need to move the price
/// at least far enough to get the desired output amount, and in the exact input case (decreasing
/// price) we need to move the price less in order to not send too much output.
/// The most precise formula for this is liquidity * sqrtPX96 / (liquidity +- amount * sqrtPX96),
/// if this is impossible because of overflow, we calculate liquidity / (liquidity / sqrtPX96 +-
/// amount).
///
/// ## Arguments
///
/// * `sqrt_price_x96`: The starting price, i.e. before accounting for the token0 delta
/// * `liquidity`: The amount of usable liquidity
/// * `amount`: How much of token0 to add or remove from virtual reserves
/// * `add`: Whether to add or remove the amount of token0
///
/// ## Returns
///
/// The price after adding or removing amount, depending on add
pub fn get_next_sqrt_price_from_amount_0_rounding_up(
    sqrt_price_x96: U160,
    liquidity: u128,
    amount: U256,
    add: bool,
) -> Result<U160, Error> {
    if amount.is_zero() {
        return Ok(sqrt_price_x96);
    }
    let sqrt_price_x96 = U256::from(sqrt_price_x96);
    let numerator_1 = U256::from(liquidity) << 96;

    if add {
        let product = amount * sqrt_price_x96;

        if product / amount == sqrt_price_x96 {
            let denominator = numerator_1 + product;
            if denominator >= numerator_1 {
                return Ok(U160::from(mul_div_rounding_up(
                    numerator_1,
                    sqrt_price_x96,
                    denominator,
                )?));
            }
        }

        Ok(U160::from(
            numerator_1.div_ceil(numerator_1 / sqrt_price_x96 + amount),
        ))
    } else {
        let product = amount * sqrt_price_x96;
        if !(product / amount == sqrt_price_x96 && numerator_1 > product) {
            Err(Error::PriceOverflow)
        } else {
            let denominator = numerator_1 - product;

            U160::uint_try_from(mul_div_rounding_up(
                numerator_1,
                sqrt_price_x96,
                denominator,
            )?)
            .map_err(|_| Error::SafeCastToU160Overflow)
        }
    }
}

/// Gets the next sqrt price given a delta of token1
///
/// Always rounds down, because in the exact output case (decreasing price) we need to move the
/// price at least far enough to get the desired output amount, and in the exact input case
/// (increasing price) we need to move the price less in order to not send too much output.
/// The formula we compute is within <1 wei of the lossless version: sqrtPX96 +- amount / liquidity
///
/// ## Arguments
///
/// * `sqrt_price_x96`: The starting price, i.e., before accounting for the token1 delta
/// * `liquidity`: The amount of usable liquidity
/// * `amount`: How much of token1 to add, or remove, from virtual reserves
/// * `add`: Whether to add, or remove, the amount of token1
///
/// ## Returns
///
/// The price after adding or removing `amount`
pub fn get_next_sqrt_price_from_amount_1_rounding_down(
    sqrt_price_x96: U160,
    liquidity: u128,
    amount: U256,
    add: bool,
) -> Result<U160, Error> {
    let sqrt_price_x96 = U256::from(sqrt_price_x96);
    let liquidity = U256::from(liquidity);
    if add {
        let quotient = if amount <= MAX_U160 {
            (amount << 96) / liquidity
        } else {
            mul_div(amount, Q96, liquidity)?
        };

        U160::uint_try_from(sqrt_price_x96 + quotient).map_err(|_| Error::SafeCastToU160Overflow)
    } else {
        let quotient = if amount <= MAX_U160 {
            (amount << 96_i32).div_ceil(liquidity)
        } else {
            mul_div_rounding_up(amount, Q96, liquidity)?
        };

        if sqrt_price_x96 > quotient {
            Ok(U160::from(sqrt_price_x96 - quotient))
        } else {
            Err(Error::InsufficientLiquidity)
        }
    }
}

/// Gets the next sqrt price given an input amount of token0 or token1
///
/// Throws if price or liquidity are 0, or if the next price is out of bounds
///
/// ## Arguments
///
/// * `sqrt_price_x96`: The starting price, i.e., before accounting for the input amount
/// * `liquidity`: The amount of usable liquidity
/// * `amount_in`: How much of token0, or token1, is being swapped in
/// * `zero_for_one`: Whether the amount in is token0 or token1
///
/// returns: The price after adding the input amount to token0 or token1
pub fn get_next_sqrt_price_from_input(
    sqrt_price_x96: U160,
    liquidity: u128,
    amount_in: U256,
    zero_for_one: bool,
) -> Result<U160, Error> {
    if sqrt_price_x96.is_zero() || liquidity.is_zero() {
        return Err(Error::InvalidPriceOrLiquidity);
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price_x96, liquidity, amount_in, true)
    } else {
        get_next_sqrt_price_from_amount_1_rounding_down(sqrt_price_x96, liquidity, amount_in, true)
    }
}

/// Gets the next sqrt price given an output amount of token0 or token1
///
/// Throws if price or liquidity are 0 or the next price is out of bounds
///
/// ## Arguments
///
/// * `sqrt_price_x96`: The starting price before accounting for the output amount
/// * `liquidity`: The amount of usable liquidity
/// * `amount_out`: How much of token0, or token1, is being swapped out
/// * `zero_for_one`: Whether the amount out is token0 or token1
///
/// returns: The price after removing the output amount of token0 or token1
pub fn get_next_sqrt_price_from_output(
    sqrt_price_x96: U160,
    liquidity: u128,
    amount_out: U256,
    zero_for_one: bool,
) -> Result<U160, Error> {
    if sqrt_price_x96.is_zero() || liquidity.is_zero() {
        return Err(Error::InvalidPriceOrLiquidity);
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount_1_rounding_down(
            sqrt_price_x96,
            liquidity,
            amount_out,
            false,
        )
    } else {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price_x96, liquidity, amount_out, false)
    }
}

#[inline]
fn sort(a: U160, b: U160) -> (U256, U256) {
    if a > b {
        (U256::from(b), U256::from(a))
    } else {
        (U256::from(a), U256::from(b))
    }
}

/// Gets the amount0 delta between two prices
///
/// Calculates liquidity / sqrt(lower) - liquidity / sqrt(upper),
/// i.e. liquidity * (sqrt(upper) - sqrt(lower)) / (sqrt(upper) * sqrt(lower))
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: A sqrt price assumed to be lower otherwise swapped
/// * `sqrt_ratio_b_x96`: Another sqrt price
/// * `liquidity`: The amount of usable liquidity
/// * `round_up`: Whether to round the amount up or down
///
/// returns: Amount of token0 required to cover a position of size liquidity between the two passed
/// prices
pub fn get_amount_0_delta(
    sqrt_ratio_a_x96: U160,
    sqrt_ratio_b_x96: U160,
    liquidity: u128,
    round_up: bool,
) -> Result<U256, Error> {
    let (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = sort(sqrt_ratio_a_x96, sqrt_ratio_b_x96);

    if sqrt_ratio_a_x96.is_zero() {
        return Err(Error::InvalidPrice);
    }

    let numerator_1 = U256::from(liquidity) << 96;
    let numerator_2 = sqrt_ratio_b_x96 - sqrt_ratio_a_x96;

    let (amount_0, rem) =
        mul_div(numerator_1, numerator_2, sqrt_ratio_b_x96)?.div_rem(sqrt_ratio_a_x96);
    let carry =
        (rem | numerator_1.mul_mod(numerator_2, sqrt_ratio_b_x96)).gt(&U256::ZERO) && round_up;
    Ok(amount_0 + U256::from_limbs([carry as u64, 0, 0, 0]))
}

/// Gets the amount1 delta between two prices
///
/// Calculates liquidity * (sqrt(upper) - sqrt(lower))
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: A sqrt price assumed to be lower otherwise swapped
/// * `sqrt_ratio_b_x96`: Another sqrt price
/// * `liquidity`: The amount of usable liquidity
/// * `round_up`: Whether to round the amount up, or down
///
/// returns: Amount of token1 required to cover a position of size liquidity between the two passed
/// prices
pub fn get_amount_1_delta(
    sqrt_ratio_a_x96: U160,
    sqrt_ratio_b_x96: U160,
    liquidity: u128,
    round_up: bool,
) -> Result<U256, Error> {
    let (sqrt_ratio_a_x96, sqrt_ratio_b_x96) = sort(sqrt_ratio_a_x96, sqrt_ratio_b_x96);

    let numerator = sqrt_ratio_b_x96 - sqrt_ratio_a_x96;
    let denominator = Q96;

    let liquidity = U256::from(liquidity);
    let amount_1 = mul_div_96(liquidity, numerator)?;
    let carry = liquidity.mul_mod(numerator, denominator).gt(&U256::ZERO) && round_up;
    Ok(amount_1 + U256::from_limbs([carry as u64, 0, 0, 0]))
}

/// Helper that gets signed token0 delta
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: A sqrt price
/// * `sqrt_ratio_b_x96`: Another sqrt price
/// * `liquidity`: The change in liquidity for which to compute the amount0 delta
///
/// returns: Amount of token0 corresponding to the passed liquidityDelta between the two prices
pub fn get_amount_0_delta_signed(
    sqrt_ratio_a_x96: U160,
    sqrt_ratio_b_x96: U160,
    liquidity: i128,
) -> Result<I256, Error> {
    let sign = !liquidity.is_negative();
    let mask = (sign as u128).wrapping_sub(1);
    let liquidity = mask ^ mask.wrapping_add_signed(liquidity);
    let mask = mask as u64;
    let mask = I256::from_limbs([mask, mask, mask, mask]);
    let amount_0 = I256::from_raw(get_amount_0_delta(
        sqrt_ratio_a_x96,
        sqrt_ratio_b_x96,
        liquidity,
        sign,
    )?);
    Ok((amount_0 ^ mask) - mask)
}

/// Helper that gets signed token1 delta
///
/// ## Arguments
///
/// * `sqrt_ratio_a_x96`: A sqrt price
/// * `sqrt_ratio_b_x96`: Another sqrt price
/// * `liquidity`: The change in liquidity for which to compute the amount1 delta
///
/// returns: Amount of token1 corresponding to the passed liquidityDelta between the two prices
pub fn get_amount_1_delta_signed(
    sqrt_ratio_a_x96: U160,
    sqrt_ratio_b_x96: U160,
    liquidity: i128,
) -> Result<I256, Error> {
    let sign = !liquidity.is_negative();
    let mask = (sign as u128).wrapping_sub(1);
    let liquidity = mask ^ mask.wrapping_add_signed(liquidity);
    let mask = mask as u64;
    let mask = I256::from_limbs([mask, mask, mask, mask]);
    let amount_1 = I256::from_raw(get_amount_1_delta(
        sqrt_ratio_a_x96,
        sqrt_ratio_b_x96,
        liquidity,
        sign,
    )?);
    Ok((amount_1 ^ mask) - mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::keccak256;
    use alloy_sol_types::SolValue;
    use uniswap_v3_math::{error::UniswapV3MathError, sqrt_price_math};

    fn pseudo_random(seed: u64) -> U256 {
        keccak256(seed.abi_encode()).into()
    }

    fn pseudo_random_128(seed: u64) -> u128 {
        let s: U256 = keccak256(seed.abi_encode()).into();
        u128::from_be_bytes(s.to_be_bytes::<32>()[..16].try_into().unwrap())
    }

    fn generate_inputs() -> Vec<(U256, u128, U256, bool)> {
        (0u64..1000)
            .map(|i| {
                (
                    pseudo_random(i),
                    pseudo_random_128(i.pow(2)),
                    pseudo_random(i.pow(3)),
                    i % 2 == 0,
                )
            })
            .collect()
    }

    fn match_u256(res: Result<U256, Error>, ref_: Result<U256, UniswapV3MathError>) {
        match res {
            Ok(res) => {
                assert_eq!(res, ref_.unwrap());
            }
            Err(_) => {
                assert!(ref_.is_err());
            }
        }
    }

    #[test]
    fn test_get_next_sqrt_price_from_input() {
        let inputs = generate_inputs();
        for (sqrt_price_x_96, liquidity, amount, add) in inputs {
            let sqrt_price_x_96 = U160::saturating_from(sqrt_price_x_96);
            let res = get_next_sqrt_price_from_input(sqrt_price_x_96, liquidity, amount, add);
            let ref_ = sqrt_price_math::get_next_sqrt_price_from_input(
                U256::from(sqrt_price_x_96),
                liquidity,
                amount,
                add,
            );
            match_u256(res.map(U256::from), ref_);
        }
    }

    #[test]
    fn test_get_next_sqrt_price_from_output() {
        let inputs = generate_inputs();
        for (sqrt_price_x_96, liquidity, amount, add) in inputs {
            let sqrt_price_x_96 = U160::saturating_from(sqrt_price_x_96);
            let res = get_next_sqrt_price_from_output(sqrt_price_x_96, liquidity, amount, add);
            let ref_ = sqrt_price_math::get_next_sqrt_price_from_output(
                U256::from(sqrt_price_x_96),
                liquidity,
                amount,
                add,
            );
            match_u256(res.map(U256::from), ref_);
        }
    }

    #[test]
    fn test_get_amount_0_delta() {
        let inputs = generate_inputs();
        for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in inputs {
            let sqrt_ratio_a_x96 = U160::saturating_from(sqrt_ratio_a_x96);
            let sqrt_ratio_b_x96 = U160::saturating_from(sqrt_ratio_b_x96);
            let res = get_amount_0_delta(sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up);
            let ref_ = sqrt_price_math::_get_amount_0_delta(
                U256::from(sqrt_ratio_a_x96),
                U256::from(sqrt_ratio_b_x96),
                liquidity,
                round_up,
            );
            match_u256(res, ref_);
        }
    }

    #[test]
    fn test_get_amount_1_delta() {
        let inputs = generate_inputs();
        for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in inputs {
            let sqrt_ratio_a_x96 = U160::saturating_from(sqrt_ratio_a_x96);
            let sqrt_ratio_b_x96 = U160::saturating_from(sqrt_ratio_b_x96);
            let res = get_amount_1_delta(sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up);
            let ref_ = sqrt_price_math::_get_amount_1_delta(
                U256::from(sqrt_ratio_a_x96),
                U256::from(sqrt_ratio_b_x96),
                liquidity,
                round_up,
            );
            match_u256(res, ref_);
        }
    }

    #[test]
    fn test_get_amount_0_delta_signed() {
        let inputs = generate_inputs();
        for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in inputs {
            let sqrt_ratio_a_x96 = U160::saturating_from(sqrt_ratio_a_x96);
            let sqrt_ratio_b_x96 = U160::saturating_from(sqrt_ratio_b_x96);
            let res =
                get_amount_0_delta_signed(sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity as i128)
                    .map(I256::into_raw);
            let ref_ = sqrt_price_math::get_amount_0_delta(
                U256::from(sqrt_ratio_a_x96),
                U256::from(sqrt_ratio_b_x96),
                liquidity as i128,
            );
            match ref_ {
                Ok(ref_) => {
                    assert_eq!(res.unwrap(), ref_.into_raw());
                }
                Err(_) => {
                    assert!(res.is_err());
                }
            }
        }
    }

    #[test]
    fn test_get_amount_1_delta_signed() {
        let inputs = generate_inputs();
        for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in inputs {
            let sqrt_ratio_a_x96 = U160::saturating_from(sqrt_ratio_a_x96);
            let sqrt_ratio_b_x96 = U160::saturating_from(sqrt_ratio_b_x96);
            let res =
                get_amount_1_delta_signed(sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity as i128)
                    .map(I256::into_raw);
            let ref_ = sqrt_price_math::get_amount_1_delta(
                U256::from(sqrt_ratio_a_x96),
                U256::from(sqrt_ratio_b_x96),
                liquidity as i128,
            );
            match ref_ {
                Ok(ref_) => {
                    assert_eq!(res.unwrap(), ref_.into_raw());
                }
                Err(_) => {
                    assert!(res.is_err());
                }
            }
        }
    }
}
