use uniswap_v3_math::{error::UniswapV3MathError, liquidity_math};

pub fn add_delta(x: u128, y: i128) -> Result<u128, UniswapV3MathError> {
    liquidity_math::add_delta(x, y)
}
