use super::{ONE, Q96, THREE, TWO};
use alloy_primitives::U256;
use uniswap_v3_math::error::UniswapV3MathError;

/// Calculates floor(a×b÷denominator) with full precision. Throws if result overflows a uint256 or
/// denominator == 0
///
/// ## Arguments
///
/// * `a`: The multiplicand
/// * `b`: The multiplier
/// * `denominator`: The divisor
///
/// returns: Result<U256, UniswapV3MathError>
pub fn mul_div(a: U256, b: U256, mut denominator: U256) -> Result<U256, UniswapV3MathError> {
    // 512-bit multiply [prod1 prod0] = a * b
    // Compute the product mod 2**256 and mod 2**256 - 1
    // then use the Chinese Remainder Theorem to reconstruct
    // the 512 bit result. The result is stored in two 256
    // variables such that product = prod1 * 2**256 + prod0
    let mm = a.mul_mod(b, U256::MAX);

    // Least significant 256 bits of the product
    let mut prod_0 = a * b;
    let mut prod_1 = mm - prod_0 - U256::from_limbs([(mm < prod_0) as u64, 0, 0, 0]);

    // Make sure the result is less than 2**256.
    // Also prevents denominator == 0
    if denominator <= prod_1 {
        if denominator.is_zero() {
            return Err(UniswapV3MathError::DenominatorIsZero);
        }
        return Err(UniswapV3MathError::DenominatorIsLteProdOne);
    }

    // Handle non-overflow cases, 256 by 256 division
    if prod_1.is_zero() {
        return Ok(prod_0 / denominator);
    }

    ///////////////////////////////////////////////
    // 512 by 256 division.
    ///////////////////////////////////////////////

    // Make division exact by subtracting the remainder from [prod1 prod0]
    // Compute remainder using mul_mod
    let remainder = a.mul_mod(b, denominator);

    // Subtract 256 bit number from 512 bit number
    prod_1 -= U256::from_limbs([(remainder > prod_0) as u64, 0, 0, 0]);
    prod_0 -= remainder;

    // Factor powers of two out of denominator
    // Compute largest power of two divisor of denominator.
    // Always >= 1.
    let mut twos = (U256::ZERO - denominator) & denominator;

    // Divide denominator by power of two
    denominator /= twos;

    // Divide [prod1 prod0] by the factors of two
    prod_0 /= twos;

    // Shift in bits from prod1 into prod0. For this we need
    // to flip `twos` such that it is 2**256 / twos.
    // If twos is zero, then it becomes one
    twos = (U256::ZERO - twos) / twos + ONE;

    prod_0 |= prod_1 * twos;

    // Invert denominator mod 2**256
    // Now that denominator is an odd number, it has an inverse
    // modulo 2**256 such that denominator * inv = 1 mod 2**256.
    // Compute the inverse by starting with a seed that is correct
    // correct for four bits. That is, denominator * inv = 1 mod 2**4
    let mut inv = (THREE * denominator) ^ TWO;

    // Now use Newton-Raphson iteration to improve the precision.
    // Thanks to Hensel's lifting lemma, this also works in modular
    // arithmetic, doubling the correct bits in each step.
    inv *= TWO - denominator * inv; // inverse mod 2**8
    inv *= TWO - denominator * inv; // inverse mod 2**16
    inv *= TWO - denominator * inv; // inverse mod 2**32
    inv *= TWO - denominator * inv; // inverse mod 2**64
    inv *= TWO - denominator * inv; // inverse mod 2**128
    inv *= TWO - denominator * inv; // inverse mod 2**256

    // Because the division is now exact we can divide by multiplying
    // with the modular inverse of denominator. This will give us the
    // correct result modulo 2**256. Since the preconditions guarantee
    // that the outcome is less than 2**256, this is the final result.
    // We don't need to compute the high bits of the result and prod1
    // is no longer required.

    Ok(prod_0 * inv)
}

/// Calculates ceil(a×b÷denominator) with full precision. Throws if result overflows a uint256 or
/// denominator == 0
///
/// ## Arguments
///
/// * `a`: The multiplicand
/// * `b`: The multiplier
/// * `denominator`: The divisor
///
/// returns: Result<U256, UniswapV3MathError>
pub fn mul_div_rounding_up(
    a: U256,
    b: U256,
    denominator: U256,
) -> Result<U256, UniswapV3MathError> {
    let result = mul_div(a, b, denominator)?;

    if a.mul_mod(b, denominator).is_zero() {
        Ok(result)
    } else if result == U256::MAX {
        Err(UniswapV3MathError::ResultIsU256MAX)
    } else {
        Ok(result + ONE)
    }
}

/// Calculates a * b / 2^96 with full precision.
pub fn mul_div_96(a: U256, b: U256) -> Result<U256, UniswapV3MathError> {
    let prod0 = a * b;
    let mm = a.mul_mod(b, U256::MAX);
    let prod1 = mm - prod0 - U256::from_limbs([(mm < prod0) as u64, 0, 0, 0]);
    if prod1.ge(&Q96) {
        return Err(UniswapV3MathError::DenominatorIsLteProdOne);
    }
    Ok((prod0 >> 96) | (prod1 << 160))
}
