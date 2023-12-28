use super::tick_math::{MAX_TICK, MIN_TICK};

/// Returns the closest tick that is nearest a given tick and usable for the given tick spacing
///
/// # Arguments
///
/// * `tick`: the target tick
/// * `tick_spacing`: the spacing of the pool
///
/// returns: i32
///
pub fn nearest_usable_tick(tick: i32, tick_spacing: i32) -> i32 {
    assert!(tick_spacing > 0, "TICK_SPACING");
    assert!((MIN_TICK..=MAX_TICK).contains(&tick), "TICK_BOUND");
    let rounded = (tick / tick_spacing) * tick_spacing;
    if rounded < MIN_TICK {
        rounded + tick_spacing
    } else if rounded > MAX_TICK {
        rounded - tick_spacing
    } else {
        rounded
    }
}
