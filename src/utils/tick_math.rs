//! ## Tick Math Library in Rust
//! This library is a Rust port of the [TickMath library](https://github.com/uniswap/v3-core/blob/main/contracts/libraries/TickMath.sol) in Solidity,
//! with custom optimizations presented in [uni-v3-lib](https://github.com/Aperture-Finance/uni-v3-lib/blob/main/src/TickMath.sol).

use super::most_significant_bit;
use alloy_primitives::{uint, U256};
use std::ops::{Shl, Shr, Sub};
use uniswap_v3_math::error::UniswapV3MathError;

pub use uniswap_v3_math::tick_math::{MAX_TICK, MIN_TICK};

/// The minimum value that can be returned from `get_sqrt_ratio_at_tick`. Equivalent to
/// `get_sqrt_ratio_at_tick(MIN_TICK)`
pub const MIN_SQRT_RATIO: U256 = uint!(4295128739_U256);
/// The maximum value that can be returned from `get_sqrt_ratio_at_tick`. Equivalent to
/// `get_sqrt_ratio_at_tick(MAX_TICK)`
pub const MAX_SQRT_RATIO: U256 = uint!(1461446703485210103287273052203988822378723970342_U256);
/// A threshold used for optimized bounds check, equals `MAX_SQRT_RATIO - MIN_SQRT_RATIO - 1`
const MAX_SQRT_RATIO_MINUS_MIN_SQRT_RATIO_MINUS_ONE: U256 =
    uint!(1461446703485210103287273052203988822374428841602_U256);

/// Returns the sqrt ratio as a Q64.96 for the given tick. The sqrt ratio is computed as
/// sqrt(1.0001)^tick
///
/// ## Arguments
///
/// * `tick`: the tick for which to compute the sqrt ratio
///
/// returns: Result<U256, UniswapV3MathError>
pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, UniswapV3MathError> {
    let abs_tick = tick.abs();

    if abs_tick > MAX_TICK {
        return Err(UniswapV3MathError::T);
    }

    // Equivalent: ratio = 2**128 / sqrt(1.0001) if abs_tick & 0x1 else 1 << 128
    let mut ratio = uint!(0xfffcb933bd6fad37aa2d162d1a59400100000000000000000000000000000000_U256)
        .shr((abs_tick & 0x1) << 7)
        & uint!(0x1ffffffffffffffffffffffffffffffff_U256);

    // Iterate through 1th to 19th bit of abs_tick because MAX_TICK < 2**20
    // Equivalent to:
    //      for i in range(1, 20):
    //          if abs_tick & 2 ** i:
    //              ratio = ratio * (2 ** 128 / 1.0001 ** (2 ** (i - 1))) / 2 ** 128
    if abs_tick & 0x2 != 0 {
        ratio = (ratio * uint!(0xfff97272373d413259a46990580e213a_U256)) >> 128;
    }
    if abs_tick & 0x4 != 0 {
        ratio = (ratio * uint!(0xfff2e50f5f656932ef12357cf3c7fdcc_U256)) >> 128
    };
    if abs_tick & 0x8 != 0 {
        ratio = (ratio * uint!(0xffe5caca7e10e4e61c3624eaa0941cd0_U256)) >> 128;
    }
    if abs_tick & 0x10 != 0 {
        ratio = (ratio * uint!(0xffcb9843d60f6159c9db58835c926644_U256)) >> 128;
    }
    if abs_tick & 0x20 != 0 {
        ratio = (ratio * uint!(0xff973b41fa98c081472e6896dfb254c0_U256)) >> 128;
    }
    if abs_tick & 0x40 != 0 {
        ratio = (ratio * uint!(0xff2ea16466c96a3843ec78b326b52861_U256)) >> 128;
    }
    if abs_tick & 0x80 != 0 {
        ratio = (ratio * uint!(0xfe5dee046a99a2a811c461f1969c3053_U256)) >> 128;
    }
    if abs_tick & 0x100 != 0 {
        ratio = (ratio * uint!(0xfcbe86c7900a88aedcffc83b479aa3a4_U256)) >> 128;
    }
    if abs_tick & 0x200 != 0 {
        ratio = (ratio * uint!(0xf987a7253ac413176f2b074cf7815e54_U256)) >> 128;
    }
    if abs_tick & 0x400 != 0 {
        ratio = (ratio * uint!(0xf3392b0822b70005940c7a398e4b70f3_U256)) >> 128;
    }
    if abs_tick & 0x800 != 0 {
        ratio = (ratio * uint!(0xe7159475a2c29b7443b29c7fa6e889d9_U256)) >> 128;
    }
    if abs_tick & 0x1000 != 0 {
        ratio = (ratio * uint!(0xd097f3bdfd2022b8845ad8f792aa5825_U256)) >> 128;
    }
    if abs_tick & 0x2000 != 0 {
        ratio = (ratio * uint!(0xa9f746462d870fdf8a65dc1f90e061e5_U256)) >> 128;
    }
    if abs_tick & 0x4000 != 0 {
        ratio = (ratio * uint!(0x70d869a156d2a1b890bb3df62baf32f7_U256)) >> 128;
    }
    if abs_tick & 0x8000 != 0 {
        ratio = (ratio * uint!(0x31be135f97d08fd981231505542fcfa6_U256)) >> 128;
    }
    if abs_tick & 0x10000 != 0 {
        ratio = (ratio * uint!(0x9aa508b5b7a84e1c677de54f3e99bc9_U256)) >> 128;
    }
    if abs_tick & 0x20000 != 0 {
        ratio = (ratio * uint!(0x5d6af8dedb81196699c329225ee604_U256)) >> 128;
    }
    if abs_tick & 0x40000 != 0 {
        ratio = (ratio * uint!(0x2216e584f5fa1ea926041bedfe98_U256)) >> 128;
    }
    if abs_tick & 0x80000 != 0 {
        ratio = (ratio * uint!(0x48a170391f7dc42444e8fa2_U256)) >> 128;
    }

    if tick > 0 {
        ratio = U256::MAX / ratio;
    }

    Ok((ratio + uint!(0xffffffff_U256)) >> 32)
}

/// Returns the tick corresponding to a given sqrt ratio,
/// s.t. get_sqrt_ratio_at_tick(tick) <= sqrt_ratio_x96 and get_sqrt_ratio_at_tick(tick + 1) >
/// sqrt_ratio_x96
///
/// ## Arguments
///
/// * `sqrt_ratio_x96`: the sqrt ratio as a Q64.96 for which to compute the tick
///
/// returns: Result<i32, UniswapV3MathError>
pub fn get_tick_at_sqrt_ratio(sqrt_ratio_x96: U256) -> Result<i32, UniswapV3MathError> {
    // Equivalent: if (sqrt_ratio_x96 < MIN_SQRT_RATIO || sqrt_ratio_x96 >= MAX_SQRT_RATIO)
    // revert("R"); if sqrt_ratio_x96 < MIN_SQRT_RATIO, the `sub` underflows and `gt` is true
    // if sqrt_ratio_x96 >= MAX_SQRT_RATIO, sqrt_ratio_x96 - MIN_SQRT_RATIO > MAX_SQRT_RATIO -
    // MAX_SQRT_RATIO - 1
    if (sqrt_ratio_x96 - MIN_SQRT_RATIO) > MAX_SQRT_RATIO_MINUS_MIN_SQRT_RATIO_MINUS_ONE {
        return Err(UniswapV3MathError::R);
    }

    // Find the most significant bit of `sqrt_ratio_x96`, 160 > msb >= 32.
    let msb = most_significant_bit(sqrt_ratio_x96);

    // 2**(msb - 95) > sqrt_ratio >= 2**(msb - 96)
    // the integer part of log_2(sqrt_ratio) * 2**64 = (msb - 96) << 64, 8.64 number
    let mut log_2_x64: U256 = U256::from_limbs([msb as u64, 0, 0, 0])
        .sub(uint!(96_U256))
        .shl(64);

    // Get the first 128 significant figures of `sqrt_ratio_x96`.
    // r = sqrt_ratio_x96 / 2**(msb - 127), where 2**128 > r >= 2**127
    // sqrt_ratio = 2**(msb - 96) * r / 2**127, in floating point math
    // Shift left first because 160 > msb >= 32. If we shift right first, we'll lose precision.
    // let r := shr(sub(msb, 31), shl(96, sqrt_ratio_x96))
    let mut r: U256 = sqrt_ratio_x96.shl(96_u8).shr(msb - 31_u8);

    fn to_u8(x: U256) -> u8 {
        x.into_limbs()[0] as u8
    }

    // Approximate `log_2_x64` to 14 binary digits after decimal
    // Check whether r >= sqrt(2) * 2**127
    // 2**256 > r**2 >= 2**254
    let mut square: U256 = r * r;
    // f = (r**2 >= 2**255)
    let mut f: U256 = square >> 255;
    // r = r**2 >> 128 if r**2 >= 2**255 else r**2 >> 127
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 63;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 62;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 61;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 60;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 59;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 58;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 57;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 56;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 55;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 54;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 53;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 52;

    square = r * r;
    f = square >> 255;
    r = square >> (to_u8(f) + 127);
    log_2_x64 |= f << 51;

    square = r * r;
    f = square >> 255;
    log_2_x64 |= f << 50;

    // sqrt_ratio = sqrt(1.0001^tick)
    // tick = log_{sqrt(1.0001)}(sqrt_ratio) = log_2(sqrt_ratio) / log_2(sqrt(1.0001))
    // 2**64 / log_2(sqrt(1.0001)) = 255738958999603826347141
    let log_sqrt10001: U256 = log_2_x64 * uint!(255738958999603826347141_U256);
    let tick_low: U256 = (log_sqrt10001 - uint!(3402992956809132418596140100660247210_U256)) >> 128;
    let tick_low: i32 = tick_low.into_limbs()[0] as i32;
    let tick_high: U256 =
        (log_sqrt10001 + uint!(291339464771989622907027621153398088495_U256)) >> 128;
    let tick_high: i32 = tick_high.into_limbs()[0] as i32;

    let tick = if tick_low == tick_high {
        tick_low
    } else {
        tick_high - (get_sqrt_ratio_at_tick(tick_high)? > sqrt_ratio_x96) as i32
    };

    Ok(tick)
}

#[cfg(test)]
mod tests {
    use super::*;

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
