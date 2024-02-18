use super::big_int_to_u256;
use alloy_primitives::U256;
use num_bigint::BigInt;
use uniswap_sdk_core::utils::sqrt::sqrt;

/// Returns the sqrt ratio as a Q64.96 corresponding to a given ratio of amount1 and amount0
///
/// ## Arguments
///
/// * `amount1`: The numerator amount i.e., the amount of token1
/// * `amount0`: The denominator amount i.e., the amount of token0
///
/// returns: U256 The sqrt ratio as a Q64.96
pub fn encode_sqrt_ratio_x96(amount1: impl Into<BigInt>, amount0: impl Into<BigInt>) -> U256 {
    let numerator: BigInt = amount1.into() << 192;
    let denominator = amount0.into();
    big_int_to_u256(sqrt(&(numerator / denominator)).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Q96;

    #[test]
    fn test_encode_sqrt_ratio_x96() {
        assert_eq!(encode_sqrt_ratio_x96(1, 1), Q96);
        assert_eq!(
            encode_sqrt_ratio_x96(100, 1),
            U256::from(792281625142643375935439503360u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(1, 100),
            U256::from(7922816251426433759354395033u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(111, 333),
            U256::from(45742400955009932534161870629u128)
        );
        assert_eq!(
            encode_sqrt_ratio_x96(333, 111),
            U256::from(137227202865029797602485611888u128)
        );
    }
}
