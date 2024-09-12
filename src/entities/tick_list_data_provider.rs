use crate::prelude::*;

/// A data provider for ticks that is backed by an in-memory array of ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickListDataProvider<I = i32>(Vec<Tick<I>>);

impl<I: TickIndex> TickListDataProvider<I> {
    #[inline]
    pub fn new(ticks: Vec<Tick<I>>, tick_spacing: I) -> Self {
        ticks.validate_list(tick_spacing);
        Self(ticks)
    }
}

impl<I: TickIndex> TickDataProvider for TickListDataProvider<I> {
    type Index = I;

    #[inline]
    fn get_tick(&self, tick: I) -> Result<&Tick<I>, Error> {
        Ok(self.0.get_tick(tick))
    }

    #[inline]
    fn next_initialized_tick_within_one_word(
        &self,
        tick: I,
        lte: bool,
        tick_spacing: I,
    ) -> Result<(I, bool), Error> {
        Ok(self
            .0
            .next_initialized_tick_within_one_word(tick, lte, tick_spacing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    static PROVIDER: Lazy<TickListDataProvider> =
        Lazy::new(|| TickListDataProvider::new(vec![Tick::new(-1, 1, -1), Tick::new(1, 1, 1)], 1));

    #[test]
    fn can_take_an_empty_list_of_ticks() {
        TickListDataProvider::<i32>::default();
    }

    #[test]
    #[should_panic(expected = "TICK_SPACING_NONZERO")]
    fn throws_for_0_tick_spacing() {
        TickListDataProvider::new(vec![], 0);
    }

    #[test]
    #[should_panic(expected = "ZERO_NET")]
    fn throws_for_uneven_tick_list() {
        TickListDataProvider::new(vec![Tick::new(-1, 1, -1), Tick::new(1, 1, 2)], 1);
    }

    #[test]
    #[should_panic(expected = "NOT_CONTAINED")]
    fn throws_if_tick_not_in_list() {
        PROVIDER.get_tick(0).unwrap();
    }

    #[test]
    fn gets_the_smallest_tick_from_the_list() {
        let tick = PROVIDER.get_tick(-1).unwrap();
        assert_eq!(tick.liquidity_net, -1);
        assert_eq!(tick.liquidity_gross, 1);
    }

    #[test]
    fn gets_the_largest_tick_from_the_list() {
        let tick = PROVIDER.get_tick(1).unwrap();
        assert_eq!(tick.liquidity_net, 1);
        assert_eq!(tick.liquidity_gross, 1);
    }
}
