use super::tick_math::{MAX_TICK, MIN_TICK};
use num_integer::Integer;

/// Returns the closest tick that is nearest a given tick and usable for the given tick spacing
///
/// ## Arguments
///
/// * `tick`: the target tick
/// * `tick_spacing`: the spacing of the pool
///
/// returns: i32
pub fn nearest_usable_tick(tick: i32, tick_spacing: i32) -> i32 {
    assert!(tick_spacing > 0, "TICK_SPACING");
    assert!((MIN_TICK..=MAX_TICK).contains(&tick), "TICK_BOUND");
    let (quotient, remainder) = tick.div_mod_floor(&tick_spacing);
    let rounded = (quotient + (remainder + tick_spacing / 2) / tick_spacing) * tick_spacing;
    if rounded < MIN_TICK {
        rounded + tick_spacing
    } else if rounded > MAX_TICK {
        rounded - tick_spacing
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "TICK_SPACING")]
    fn panics_if_tick_spacing_is_0() {
        nearest_usable_tick(1, 0);
    }

    #[test]
    #[should_panic(expected = "TICK_SPACING")]
    fn panics_if_tick_spacing_is_negative() {
        nearest_usable_tick(1, -5);
    }

    #[test]
    #[should_panic(expected = "TICK_BOUND")]
    fn panics_if_tick_is_greater_than_max() {
        nearest_usable_tick(MAX_TICK + 1, 1);
    }

    #[test]
    #[should_panic(expected = "TICK_BOUND")]
    fn panics_if_tick_is_less_than_min() {
        nearest_usable_tick(MIN_TICK - 1, 1);
    }

    #[test]
    fn rounds_at_positive_half() {
        assert_eq!(nearest_usable_tick(5, 10), 10);
    }

    #[test]
    fn rounds_down_below_positive_half() {
        assert_eq!(nearest_usable_tick(4, 10), 0);
    }

    #[test]
    fn rounds_down_for_negative_half() {
        assert_eq!(nearest_usable_tick(-5, 10), 0);
    }

    #[test]
    fn rounds_up_for_negative_above_half() {
        assert_eq!(nearest_usable_tick(-6, 10), -10);
    }

    #[test]
    fn cannot_round_past_min_tick() {
        assert_eq!(
            nearest_usable_tick(MIN_TICK, MAX_TICK / 2 + 100),
            -(MAX_TICK / 2 + 100)
        );
    }

    #[test]
    fn cannot_round_past_max_tick() {
        assert_eq!(
            nearest_usable_tick(MAX_TICK, MAX_TICK / 2 + 100),
            MAX_TICK / 2 + 100
        );
    }
}
