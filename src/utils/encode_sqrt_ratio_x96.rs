use num_bigint::BigInt;
use uniswap_sdk_core_rust::utils::sqrt::sqrt;

/// Returns the sqrt ratio as a Q64.96 corresponding to a given ratio of amount1 and amount0
///
/// # Arguments
///
/// * `amount1`: The numerator amount i.e., the amount of token1
/// * `amount0`: The denominator amount i.e., the amount of token0
///
/// returns: BigInt The sqrt ratio
///
pub fn encode_sqrt_ratio_x96(amount1: impl Into<BigInt>, amount0: impl Into<BigInt>) -> BigInt {
    let numerator: BigInt = amount1.into() << 192;
    let denominator = amount0.into();
    sqrt(&(numerator / denominator))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_sqrt_ratio_x96() {
        let q96 = BigInt::from(1) << 96;

        assert_eq!(encode_sqrt_ratio_x96(1, 1), q96);
        assert_eq!(
            encode_sqrt_ratio_x96(100, 1),
            792281625142643375935439503360u128.into()
        );
        assert_eq!(
            encode_sqrt_ratio_x96(1, 100),
            7922816251426433759354395033u128.into()
        );
        assert_eq!(
            encode_sqrt_ratio_x96(111, 333),
            45742400955009932534161870629u128.into()
        );
        assert_eq!(
            encode_sqrt_ratio_x96(333, 111),
            137227202865029797602485611888u128.into()
        );
    }
}
