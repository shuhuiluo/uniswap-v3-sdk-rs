use alloy_primitives::U256;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeeGrowthOutside {
    pub fee_growth_outside0_x128: U256,
    pub fee_growth_outside1_x128: U256,
}

pub fn get_fee_growth_inside(
    lower: FeeGrowthOutside,
    upper: FeeGrowthOutside,
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    fee_growth_global0_x128: U256,
    fee_growth_global1_x128: U256,
) -> (U256, U256) {
    let fee_growth_inside0_x128: U256;
    let fee_growth_inside1_x128: U256;
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
