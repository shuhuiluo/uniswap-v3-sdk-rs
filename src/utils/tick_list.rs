use crate::entities::TickTrait;
use num_integer::Integer;

/// Utility methods for interacting with sorted lists of ticks
pub trait TickList {
    type Tick;

    fn validate_list(&self, tick_spacing: i32);

    fn is_below_smallest(&self, tick: i32) -> bool;

    fn is_at_or_above_largest(&self, tick: i32) -> bool;

    fn get_tick(&self, index: i32) -> &Self::Tick;

    /// Finds the largest tick in the list of ticks that is less than or equal to tick
    ///
    /// ## Arguments
    ///
    /// * `tick`: tick to find the largest tick that is less than or equal to tick
    ///
    /// returns: usize
    fn binary_search_by_tick(&self, tick: i32) -> usize;

    fn next_initialized_tick(&self, tick: i32, lte: bool) -> &Self::Tick;

    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> (i32, bool);
}

impl<T: TickTrait> TickList for [T] {
    type Tick = T;

    fn validate_list(&self, tick_spacing: i32) {
        assert!(tick_spacing > 0, "TICK_SPACING_NONZERO");
        assert!(
            self.iter().all(|x| x.index() % tick_spacing == 0),
            "TICK_SPACING"
        );
        for i in 1..self.len() {
            if self[i] < self[i - 1] {
                panic!("SORTED");
            }
        }
        assert_eq!(
            self.iter().fold(0, |acc, x| acc + x.liquidity_net()),
            0,
            "ZERO_NET"
        );
    }

    fn is_below_smallest(&self, tick: i32) -> bool {
        assert!(!self.is_empty(), "LENGTH");
        tick < self[0].index()
    }

    fn is_at_or_above_largest(&self, tick: i32) -> bool {
        assert!(!self.is_empty(), "LENGTH");
        tick >= self.last().unwrap().index()
    }

    fn get_tick(&self, index: i32) -> &T {
        let i = Self::binary_search_by_tick(self, index);
        let tick = &self[i];
        assert_eq!(tick.index(), index, "NOT_CONTAINED");
        tick
    }

    fn binary_search_by_tick(&self, tick: i32) -> usize {
        assert!(!Self::is_below_smallest(self, tick), "BELOW_SMALLEST");
        let mut l = 0;
        let mut r = self.len() - 1;

        loop {
            let i = (l + r) / 2;
            if self[i].index() <= tick && (i == self.len() - 1 || self[i + 1].index() > tick) {
                return i;
            }
            if self[i].index() < tick {
                l = i + 1;
            } else {
                r = i - 1;
            }
        }
    }

    fn next_initialized_tick(&self, tick: i32, lte: bool) -> &T {
        if lte {
            assert!(!Self::is_below_smallest(self, tick), "BELOW_SMALLEST");
            if Self::is_at_or_above_largest(self, tick) {
                return self.last().unwrap();
            }
            let index = Self::binary_search_by_tick(self, tick);
            &self[index]
        } else {
            assert!(
                !Self::is_at_or_above_largest(self, tick),
                "AT_OR_ABOVE_LARGEST"
            );
            if Self::is_below_smallest(self, tick) {
                return &self[0];
            }
            let index = Self::binary_search_by_tick(self, tick);
            &self[index + 1]
        }
    }

    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> (i32, bool) {
        let (compressed, _) = tick.div_mod_floor(&tick_spacing);
        if lte {
            let word_pos = compressed >> 8;
            let minimum = (word_pos << 8) * tick_spacing;

            if Self::is_below_smallest(self, tick) {
                return (minimum, false);
            }
            let index = Self::next_initialized_tick(self, tick, lte).index();
            let next_initialized_tick = minimum.max(index);
            (next_initialized_tick, next_initialized_tick == index)
        } else {
            let word_pos = (compressed + 1) >> 8;
            let maximum = ((word_pos << 8) + 255) * tick_spacing;
            if Self::is_at_or_above_largest(self, tick) {
                return (maximum, false);
            }
            let index = Self::next_initialized_tick(self, tick, lte).index();
            let next_initialized_tick = maximum.min(index);
            (next_initialized_tick, next_initialized_tick == index)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entities::Tick,
        utils::{MAX_TICK, MIN_TICK},
    };

    const LOW_TICK: Tick = Tick::new(MIN_TICK + 1, 10, 10);
    const MID_TICK: Tick = Tick::new(0, 5, -5);
    const HIGH_TICK: Tick = Tick::new(MAX_TICK - 1, 5, -5);
    const TICKS: [Tick; 3] = [LOW_TICK, MID_TICK, HIGH_TICK];

    #[test]
    fn test_impl_for_vec() {
        let ticks = [LOW_TICK, MID_TICK, HIGH_TICK];
        assert_eq!(ticks.binary_search_by_tick(-1), 0);
        assert_eq!(ticks.binary_search_by_tick(0), 1);
        assert_eq!(ticks.binary_search_by_tick(1), 1);
        assert_eq!(ticks.binary_search_by_tick(MAX_TICK), 2);
    }

    #[test]
    #[should_panic(expected = "ZERO_NET")]
    fn test_validate_list_zero_net() {
        [LOW_TICK].validate_list(1);
    }

    #[test]
    #[should_panic(expected = "SORTED")]
    fn test_validate_list_unsorted() {
        [HIGH_TICK, LOW_TICK, MID_TICK].validate_list(1);
    }

    #[test]
    #[should_panic(expected = "TICK_SPACING")]
    fn test_validate_list_tick_spacing() {
        [HIGH_TICK, LOW_TICK, MID_TICK].validate_list(1337);
    }

    #[test]
    fn test_is_below_smallest() {
        assert!(TICKS.is_below_smallest(MIN_TICK));
        assert!(!TICKS.is_below_smallest(MIN_TICK + 1));
    }

    #[test]
    fn test_is_at_or_above_largest() {
        assert!(!TICKS.is_at_or_above_largest(MAX_TICK - 2));
        assert!(TICKS.is_at_or_above_largest(MAX_TICK - 1));
    }

    #[test]
    #[should_panic(expected = "BELOW_SMALLEST")]
    fn test_next_initialized_tick_low_lte_true() {
        TICKS.next_initialized_tick(MIN_TICK, true);
    }

    #[test]
    fn test_next_initialized_tick_low_lte_true_2() {
        assert_eq!(TICKS.next_initialized_tick(MIN_TICK + 1, true), &LOW_TICK);
        assert_eq!(TICKS.next_initialized_tick(MIN_TICK + 2, true), &LOW_TICK);
    }

    #[test]
    fn test_next_initialized_tick_low_lte_false() {
        assert_eq!(TICKS.next_initialized_tick(MIN_TICK, false), &LOW_TICK);
        assert_eq!(TICKS.next_initialized_tick(MIN_TICK + 1, false), &MID_TICK);
    }

    #[test]
    fn test_next_initialized_tick_mid_lte_true() {
        assert_eq!(TICKS.next_initialized_tick(0, true), &MID_TICK);
        assert_eq!(TICKS.next_initialized_tick(1, true), &MID_TICK);
    }

    #[test]
    fn test_next_initialized_tick_mid_lte_false() {
        assert_eq!(TICKS.next_initialized_tick(-1, false), &MID_TICK);
        assert_eq!(TICKS.next_initialized_tick(1, false), &HIGH_TICK);
    }

    #[test]
    fn test_next_initialized_tick_high_lte_true() {
        assert_eq!(TICKS.next_initialized_tick(MAX_TICK - 1, true), &HIGH_TICK);
        assert_eq!(TICKS.next_initialized_tick(MAX_TICK, true), &HIGH_TICK);
    }

    #[test]
    #[should_panic(expected = "AT_OR_ABOVE_LARGEST")]
    fn test_next_initialized_tick_high_lte_false() {
        TICKS.next_initialized_tick(MAX_TICK - 1, false);
    }

    #[test]
    fn test_next_initialized_tick_high_lte_false_2() {
        assert_eq!(TICKS.next_initialized_tick(MAX_TICK - 2, false), &HIGH_TICK);
        assert_eq!(TICKS.next_initialized_tick(MAX_TICK - 3, false), &HIGH_TICK);
    }

    #[test]
    fn test_next_initialized_tick_within_one_word_lte_true() {
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-257, true, 1),
            (-512, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-256, true, 1),
            (-256, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-1, true, 1),
            (-256, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(0, true, 1),
            (0, true)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(1, true, 1),
            (0, true)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(255, true, 1),
            (0, true)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(256, true, 1),
            (256, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(257, true, 1),
            (256, false)
        );
    }

    #[test]
    fn test_next_initialized_tick_within_one_word_lte_false() {
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-258, false, 1),
            (-257, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-257, false, 1),
            (-1, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-256, false, 1),
            (-1, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-2, false, 1),
            (-1, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(-1, false, 1),
            (0, true)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(0, false, 1),
            (255, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(1, false, 1),
            (255, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(254, false, 1),
            (255, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(255, false, 1),
            (511, false)
        );
        assert_eq!(
            TICKS.next_initialized_tick_within_one_word(256, false, 1),
            (511, false)
        );
    }

    #[test]
    fn test_next_initialized_tick_within_one_word_tick_spacing_gt_1() {
        let ticks = [
            Tick {
                index: 0,
                liquidity_net: 0,
                liquidity_gross: 0,
            },
            Tick {
                index: 511,
                liquidity_net: 0,
                liquidity_gross: 0,
            },
        ];
        assert_eq!(
            ticks.next_initialized_tick_within_one_word(0, false, 1),
            (255, false)
        );
        assert_eq!(
            ticks.next_initialized_tick_within_one_word(0, false, 2),
            (510, false)
        );
    }
}
