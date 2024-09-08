use super::{MAX_TICK_I32 as MAX_TICK, MIN_TICK_I32 as MIN_TICK};
use alloy_primitives::aliases::I24;
use num_integer::Integer;

/// Returns the closest tick that is nearest a given tick and usable for the given tick spacing
///
/// ## Arguments
///
/// * `tick`: the target tick
/// * `tick_spacing`: the spacing of the pool
///
/// ## Returns
///
/// The closest tick to the input tick that is usable for the given tick spacing
// TODO: use [`TickIndex`]
pub fn nearest_usable_tick(tick: I24, tick_spacing: I24) -> I24 {
    let tick = tick.as_i32();
    let tick_spacing = tick_spacing.as_i32();
    assert!(tick_spacing > 0, "TICK_SPACING");
    assert!((MIN_TICK..=MAX_TICK).contains(&tick), "TICK_BOUND");
    let (quotient, remainder) = tick.div_mod_floor(&tick_spacing);
    let rounded = (quotient + (remainder + tick_spacing / 2) / tick_spacing) * tick_spacing;
    I24::try_from(if rounded < MIN_TICK {
        rounded + tick_spacing
    } else if rounded > MAX_TICK {
        rounded - tick_spacing
    } else {
        rounded
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        nearest_usable_tick,
        tick_math::{MAX_TICK, MIN_TICK},
    };
    use alloy_primitives::aliases::I24;

    const FIVE: I24 = I24::from_limbs([5]);
    const TEN: I24 = I24::from_limbs([10]);

    #[test]
    #[should_panic(expected = "TICK_SPACING")]
    fn panics_if_tick_spacing_is_0() {
        nearest_usable_tick(I24::ONE, I24::ZERO);
    }

    #[test]
    #[should_panic(expected = "TICK_SPACING")]
    fn panics_if_tick_spacing_is_negative() {
        nearest_usable_tick(I24::ONE, -FIVE);
    }

    #[test]
    #[should_panic(expected = "TICK_BOUND")]
    fn panics_if_tick_is_greater_than_max() {
        nearest_usable_tick(MAX_TICK + I24::ONE, I24::ONE);
    }

    #[test]
    #[should_panic(expected = "TICK_BOUND")]
    fn panics_if_tick_is_less_than_min() {
        nearest_usable_tick(MIN_TICK - I24::ONE, I24::ONE);
    }

    #[test]
    fn rounds_at_positive_half() {
        assert_eq!(nearest_usable_tick(FIVE, TEN), TEN);
    }

    #[test]
    fn rounds_down_below_positive_half() {
        assert_eq!(nearest_usable_tick(I24::from_limbs([4]), TEN), I24::ZERO);
    }

    #[test]
    fn rounds_down_for_negative_half() {
        assert_eq!(nearest_usable_tick(-FIVE, TEN), I24::ZERO);
    }

    #[test]
    fn rounds_up_for_negative_above_half() {
        assert_eq!(nearest_usable_tick(-I24::from_limbs([6]), TEN), -TEN);
    }

    #[test]
    fn cannot_round_past_min_tick() {
        let tick = MAX_TICK / I24::from_limbs([2]) + I24::from_limbs([100]);
        assert_eq!(nearest_usable_tick(MIN_TICK, tick), -tick);
    }

    #[test]
    fn cannot_round_past_max_tick() {
        let tick = MAX_TICK / I24::from_limbs([2]) + I24::from_limbs([100]);
        assert_eq!(nearest_usable_tick(MAX_TICK, tick), tick);
    }
}
