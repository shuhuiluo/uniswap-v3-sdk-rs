use alloy_primitives::U256;
use uniswap_v3_math::{
    error::UniswapV3MathError,
    full_math,
    utils::{ruint_to_u256, u256_to_ruint},
};

pub fn mul_div(a: U256, b: U256, denominator: U256) -> Result<U256, UniswapV3MathError> {
    let res = full_math::mul_div(
        ruint_to_u256(a),
        ruint_to_u256(b),
        ruint_to_u256(denominator),
    )?;
    Ok(u256_to_ruint(res))
}

pub fn mul_div_rounding_up(
    a: U256,
    b: U256,
    denominator: U256,
) -> Result<U256, UniswapV3MathError> {
    let res = full_math::mul_div_rounding_up(
        ruint_to_u256(a),
        ruint_to_u256(b),
        ruint_to_u256(denominator),
    )?;
    Ok(u256_to_ruint(res))
}
