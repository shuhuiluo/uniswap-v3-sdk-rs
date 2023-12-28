use alloy_primitives::U256;
use uniswap_v3_math::{error::UniswapV3MathError, tick_math};

pub use uniswap_v3_math::tick_math::{MAX_TICK, MIN_TICK};

pub const MIN_SQRT_RATIO: U256 = U256::from_limbs([4295128739, 0, 0, 0]);
pub const MAX_SQRT_RATIO: U256 =
    U256::from_limbs([6743328256752651558, 17280870778742802505, 4294805859, 0]);

pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, UniswapV3MathError> {
    let res = tick_math::get_sqrt_ratio_at_tick(tick)?;
    Ok(U256::from_limbs_slice(res.as_ref()))
}

pub fn get_tick_at_sqrt_ratio(sqrt_ratio_x96: U256) -> Result<i32, UniswapV3MathError> {
    let be_bytes = sqrt_ratio_x96.to_be_bytes();
    tick_math::get_tick_at_sqrt_ratio(be_bytes.into())
}

#[cfg(test)]
mod tests {}
