use super::Q128;
use alloy_primitives::Uint;

/// Computes the amount of fees owed to a position
#[inline]
#[must_use]
pub fn get_tokens_owed<const BITS: usize, const LIMBS: usize>(
    fee_growth_inside_0_last_x128: Uint<BITS, LIMBS>,
    fee_growth_inside_1_last_x128: Uint<BITS, LIMBS>,
    liquidity: u128,
    fee_growth_inside_0_x128: Uint<BITS, LIMBS>,
    fee_growth_inside_1_x128: Uint<BITS, LIMBS>,
) -> (Uint<BITS, LIMBS>, Uint<BITS, LIMBS>) {
    let liquidity = Uint::from(liquidity);
    let q128 = Uint::from(Q128);
    let tokens_owed_0 =
        (fee_growth_inside_0_x128 - fee_growth_inside_0_last_x128) * liquidity / q128;
    let tokens_owed_1 =
        (fee_growth_inside_1_x128 - fee_growth_inside_1_last_x128) * liquidity / q128;
    (tokens_owed_0, tokens_owed_1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::U256;

    #[test]
    fn test_get_tokens_owed() {
        let (tokens_owed_0, tokens_owed_1) = get_tokens_owed(U256::ZERO, U256::ZERO, 1, Q128, Q128);
        assert_eq!(tokens_owed_0, U256::from(1));
        assert_eq!(tokens_owed_1, U256::from(1));
    }
}
