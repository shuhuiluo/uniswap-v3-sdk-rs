use crate::utils::{MAX_TICK, MIN_TICK};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick {
    pub index: i32,
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
}

pub trait TickTrait: PartialOrd {
    fn index(&self) -> i32;

    fn liquidity_gross(&self) -> u128;

    fn liquidity_net(&self) -> i128;
}

impl TickTrait for Tick {
    fn index(&self) -> i32 {
        self.index
    }

    fn liquidity_gross(&self) -> u128 {
        self.liquidity_gross
    }

    fn liquidity_net(&self) -> i128 {
        self.liquidity_net
    }
}

impl Tick {
    pub const fn new(index: i32, liquidity_gross: u128, liquidity_net: i128) -> Self {
        assert!(index >= MIN_TICK && index <= MAX_TICK, "TICK");
        Self {
            index,
            liquidity_gross,
            liquidity_net,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "TICK")]
    fn test_tick_below_min_tick() {
        Tick::new(MIN_TICK - 1, 0, 0);
    }

    #[test]
    #[should_panic(expected = "TICK")]
    fn test_tick_above_max_tick() {
        Tick::new(MAX_TICK + 1, 0, 0);
    }
}
