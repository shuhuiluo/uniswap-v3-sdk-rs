use alloy_primitives::Uint;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FeeGrowthOutside<const BITS: usize, const LIMBS: usize> {
    pub fee_growth_outside0_x128: Uint<BITS, LIMBS>,
    pub fee_growth_outside1_x128: Uint<BITS, LIMBS>,
}

#[inline]
#[allow(clippy::needless_pass_by_value)]
pub fn get_fee_growth_inside<const BITS: usize, const LIMBS: usize, T: PartialOrd>(
    lower: FeeGrowthOutside<BITS, LIMBS>,
    upper: FeeGrowthOutside<BITS, LIMBS>,
    tick_lower: T,
    tick_upper: T,
    tick_current: T,
    fee_growth_global0_x128: Uint<BITS, LIMBS>,
    fee_growth_global1_x128: Uint<BITS, LIMBS>,
) -> (Uint<BITS, LIMBS>, Uint<BITS, LIMBS>) {
    let fee_growth_inside0_x128;
    let fee_growth_inside1_x128;
    if tick_current < tick_lower {
        fee_growth_inside0_x128 = lower.fee_growth_outside0_x128 - upper.fee_growth_outside0_x128;
        fee_growth_inside1_x128 = lower.fee_growth_outside1_x128 - upper.fee_growth_outside1_x128;
    } else if tick_current >= tick_upper {
        fee_growth_inside0_x128 = upper.fee_growth_outside0_x128 - lower.fee_growth_outside0_x128;
        fee_growth_inside1_x128 = upper.fee_growth_outside1_x128 - lower.fee_growth_outside1_x128;
    } else {
        fee_growth_inside0_x128 = fee_growth_global0_x128
            - lower.fee_growth_outside0_x128
            - upper.fee_growth_outside0_x128;
        fee_growth_inside1_x128 = fee_growth_global1_x128
            - lower.fee_growth_outside1_x128
            - upper.fee_growth_outside1_x128;
    }
    (fee_growth_inside0_x128, fee_growth_inside1_x128)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Q128;
    use alloy_primitives::U256;

    #[test]
    fn test_zero() {
        let (fee_growth_inside0_x128, fee_growth_inside1_x128) = get_fee_growth_inside(
            FeeGrowthOutside::default(),
            FeeGrowthOutside::default(),
            -1,
            1,
            0,
            U256::ZERO,
            U256::ZERO,
        );
        assert_eq!(fee_growth_inside0_x128, U256::ZERO);
        assert_eq!(fee_growth_inside1_x128, U256::ZERO);
    }

    #[test]
    fn test_non_zero_all_inside() {
        let (fee_growth_inside0_x128, fee_growth_inside1_x128) = get_fee_growth_inside(
            FeeGrowthOutside::default(),
            FeeGrowthOutside::default(),
            -1,
            1,
            0,
            Q128,
            Q128,
        );
        assert_eq!(fee_growth_inside0_x128, Q128);
        assert_eq!(fee_growth_inside1_x128, Q128);
    }

    #[test]
    fn test_non_zero_some_outside() {
        let q127 = Q128 >> 1;
        let lower = FeeGrowthOutside {
            fee_growth_outside0_x128: q127,
            fee_growth_outside1_x128: q127,
        };
        let upper = FeeGrowthOutside::default();
        let (fee_growth_inside0_x128, fee_growth_inside1_x128) =
            get_fee_growth_inside(lower, upper, -1, 1, 0, Q128, Q128);
        assert_eq!(fee_growth_inside0_x128, q127);
        assert_eq!(fee_growth_inside1_x128, q127);
    }
}
