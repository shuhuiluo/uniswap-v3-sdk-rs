use crate::prelude::*;
use alloy_primitives::{I256, U256};
use uniswap_v3_math::error::UniswapV3MathError;

/// Computes the result of swapping some amount in, or amount out, given the parameters of the swap
///
/// The fee, plus the amount in, will never exceed the amount remaining if the swap's
/// `amountSpecified` is positive
///
/// ## Arguments
///
/// * `sqrt_ratio_current_x96`: The current sqrt price of the pool
/// * `sqrt_ratio_target_x96`: The price that cannot be exceeded, from which the direction of the
///   swap is inferred
/// * `liquidity`: The usable liquidity
/// * `amount_remaining`: How much input or output amount is remaining to be swapped in/out
/// * `fee_pips`: The fee taken from the input amount, expressed in hundredths of a bip
///
/// ## Returns
///
/// * `sqrt_ratio_next_x96`: The price after swapping the amount in/out, not to exceed the price
///   target
/// * `amount_in`: The amount to be swapped in, of either token0 or token1, based on the direction
///   of the swap
/// * `amount_out`: The amount to be received, of either token0 or token1, based on the direction of
///   the swap
/// * `fee_amount`: The amount of input that will be taken as a fee
pub fn compute_swap_step(
    sqrt_ratio_current_x96: U256,
    sqrt_ratio_target_x96: U256,
    liquidity: u128,
    amount_remaining: I256,
    fee_pips: u32,
) -> Result<(U256, U256, U256, U256), UniswapV3MathError> {
    const MAX_FEE: U256 = U256::from_limbs([1000000, 0, 0, 0]);
    let fee_pips = U256::from_limbs([fee_pips as u64, 0, 0, 0]);
    let fee_complement = MAX_FEE - fee_pips;
    let zero_for_one = sqrt_ratio_current_x96 >= sqrt_ratio_target_x96;
    let exact_in = amount_remaining >= I256::ZERO;

    let sqrt_ratio_next_x96: U256;
    let mut amount_in: U256;
    let mut amount_out: U256;
    let fee_amount: U256;
    if exact_in {
        let amount_remaining_abs = amount_remaining.into_raw();
        let amount_remaining_less_fee = mul_div(amount_remaining_abs, fee_complement, MAX_FEE)?;

        amount_in = if zero_for_one {
            get_amount_0_delta(
                sqrt_ratio_target_x96,
                sqrt_ratio_current_x96,
                liquidity,
                true,
            )?
        } else {
            get_amount_1_delta(
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                true,
            )?
        };

        if amount_remaining_less_fee >= amount_in {
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96;
            fee_amount = mul_div_rounding_up(amount_in, fee_pips, fee_complement)?;
        } else {
            amount_in = amount_remaining_less_fee;
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_input(
                sqrt_ratio_current_x96,
                liquidity,
                amount_in,
                zero_for_one,
            )?;
            fee_amount = amount_remaining_abs - amount_in;
        }

        amount_out = if zero_for_one {
            get_amount_1_delta(
                sqrt_ratio_next_x96,
                sqrt_ratio_current_x96,
                liquidity,
                false,
            )?
        } else {
            get_amount_0_delta(
                sqrt_ratio_current_x96,
                sqrt_ratio_next_x96,
                liquidity,
                false,
            )?
        };
    } else {
        let amount_remaining_abs = (-amount_remaining).into_raw();

        amount_out = if zero_for_one {
            get_amount_1_delta(
                sqrt_ratio_target_x96,
                sqrt_ratio_current_x96,
                liquidity,
                false,
            )?
        } else {
            get_amount_0_delta(
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                false,
            )?
        };

        if amount_remaining_abs >= amount_out {
            sqrt_ratio_next_x96 = sqrt_ratio_target_x96;
        } else {
            amount_out = amount_remaining_abs;
            sqrt_ratio_next_x96 = get_next_sqrt_price_from_output(
                sqrt_ratio_current_x96,
                liquidity,
                amount_out,
                zero_for_one,
            )?;
        }

        amount_in = if zero_for_one {
            get_amount_0_delta(sqrt_ratio_next_x96, sqrt_ratio_current_x96, liquidity, true)?
        } else {
            get_amount_1_delta(sqrt_ratio_current_x96, sqrt_ratio_next_x96, liquidity, true)?
        };
        fee_amount = mul_div_rounding_up(amount_in, fee_pips, fee_complement)?;
    }

    Ok((sqrt_ratio_next_x96, amount_in, amount_out, fee_amount))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compute_swap_step() {
        // TODO: Add more tests
    }
}
