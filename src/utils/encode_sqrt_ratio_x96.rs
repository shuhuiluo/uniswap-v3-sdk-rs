use alloy_primitives::Uint;
use bnum::cast::CastFrom;
use fastnum::I1024;
use uniswap_sdk_core::prelude::*;

/// Returns the sqrt ratio as a Q64.96 corresponding to a given ratio of `amount1` and `amount0`.
///
/// ## Arguments
///
/// * `amount1`: The numerator amount i.e., the amount of token1
/// * `amount0`: The denominator amount i.e., the amount of token0
///
/// ## Returns
///
/// The sqrt ratio as a Q64.96
#[inline]
pub fn encode_sqrt_ratio_x96<const BITS: usize, const LIMBS: usize>(
    amount1: impl Into<BigInt>,
    amount0: impl Into<BigInt>,
) -> Uint<BITS, LIMBS> {
    let numerator = I1024::cast_from(amount1.into()) << 192;
    let denominator = I1024::cast_from(amount0.into());
    Uint::from_big_int(sqrt(BigInt::cast_from(numerator / denominator)).unwrap())
}

#[cfg(test)]
mod tests {
    use crate::utils::{encode_sqrt_ratio_x96, Q96};
    use alloy_primitives::U256;

    #[test]
    fn test_encode_sqrt_ratio_x96() {
        assert_eq!(encode_sqrt_ratio_x96(1, 1), Q96);
        assert_eq!(
            encode_sqrt_ratio_x96(100, 1),
            U256::from(792281625142643375935439503360_u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(1, 100),
            U256::from(7922816251426433759354395033_u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(111, 333),
            U256::from(45742400955009932534161870629_u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(333, 111),
            U256::from(137227202865029797602485611888_u128)
        );
    }
}
