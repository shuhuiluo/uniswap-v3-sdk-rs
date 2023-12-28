use alloy_primitives::U256;
use uniswap_v3_math::{error::UniswapV3MathError, tick_math};

pub use uniswap_v3_math::tick_math::{MAX_TICK, MIN_TICK};

pub const MIN_SQRT_RATIO: U256 = U256::from_limbs([4295128739, 0, 0, 0]);
pub const MAX_SQRT_RATIO: U256 =
    U256::from_limbs([6743328256752651558, 17280870778742802505, 4294805859, 0]);

pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, UniswapV3MathError> {
    // TODO: optimize
    let res = tick_math::get_sqrt_ratio_at_tick(tick)?;
    Ok(U256::from_limbs_slice(res.as_ref()))
}

pub fn get_tick_at_sqrt_ratio(sqrt_ratio_x96: U256) -> Result<i32, UniswapV3MathError> {
    let be_bytes = sqrt_ratio_x96.to_be_bytes();
    // TODO: optimize
    tick_math::get_tick_at_sqrt_ratio(be_bytes.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Shl;

    #[test]
    fn min_tick() {
        assert_eq!(MIN_TICK, -887272);
    }

    #[test]
    fn max_tick() {
        assert_eq!(MAX_TICK, 887272);
    }

    #[test]
    #[should_panic(expected = "T")]
    fn get_sqrt_ratio_at_tick_throws_for_tick_too_small() {
        get_sqrt_ratio_at_tick(MIN_TICK - 1).unwrap();
    }

    #[test]
    #[should_panic(expected = "T")]
    fn get_sqrt_ratio_at_tick_throws_for_tick_too_large() {
        get_sqrt_ratio_at_tick(MAX_TICK + 1).unwrap();
    }

    #[test]
    fn returns_correct_value_for_min_tick() {
        assert_eq!(get_sqrt_ratio_at_tick(MIN_TICK).unwrap(), MIN_SQRT_RATIO);
    }

    #[test]
    fn returns_correct_value_for_tick_zero() {
        assert_eq!(get_sqrt_ratio_at_tick(0).unwrap(), U256::from(1).shl(96));
    }

    #[test]
    fn returns_correct_value_for_max_tick() {
        assert_eq!(get_sqrt_ratio_at_tick(MAX_TICK).unwrap(), MAX_SQRT_RATIO);
    }

    #[test]
    fn returns_correct_value_for_sqrt_ratio_at_min_tick() {
        assert_eq!(get_tick_at_sqrt_ratio(MIN_SQRT_RATIO).unwrap(), MIN_TICK);
    }

    #[test]
    fn returns_correct_value_for_sqrt_ratio_at_max_tick() {
        assert_eq!(
            get_tick_at_sqrt_ratio(MAX_SQRT_RATIO - U256::from(1u32)).unwrap(),
            MAX_TICK - 1
        );
    }
}
