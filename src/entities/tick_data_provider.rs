use crate::prelude::*;

/// Provides information about ticks
pub trait TickDataProvider: Clone {
    type Tick;

    /// Return information corresponding to a specific tick
    ///
    /// ## Arguments
    ///
    /// * `tick`: The tick to load
    ///
    /// returns: Result<&Self::Tick, Error>
    fn get_tick(&self, tick: i32) -> Result<&Self::Tick, Error>;

    /// Return the next tick that is initialized within a single word
    ///
    /// ## Arguments
    ///
    /// * `tick`: The current tick
    /// * `lte`: Whether the next tick should be lte the current tick
    /// * `tick_spacing`: The tick spacing of the pool
    ///
    /// returns: Result<(i32, bool), Error>
    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> Result<(i32, bool), Error>;
}

/// This tick data provider does not know how to fetch any tick data. It throws whenever it is
/// required. Useful if you do not need to load tick data for your use case.
#[derive(Clone, Copy, Debug)]
pub struct NoTickDataProvider;

impl TickDataProvider for NoTickDataProvider {
    type Tick = Tick;

    fn get_tick(&self, _: i32) -> Result<&Tick, Error> {
        Err(Error::NoTickDataError)
    }

    fn next_initialized_tick_within_one_word(
        &self,
        _: i32,
        _: bool,
        _: i32,
    ) -> Result<(i32, bool), Error> {
        Err(Error::NoTickDataError)
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::*;

    #[test]
    fn test_no_tick_data_provider() {
        let tick_data_provider = NoTickDataProvider;
        assert_eq!(
            tick_data_provider.get_tick(0).unwrap_err().to_string(),
            Error::NoTickDataError.to_string()
        );
        assert_eq!(
            tick_data_provider
                .next_initialized_tick_within_one_word(0, false, 1)
                .unwrap_err()
                .to_string(),
            Error::NoTickDataError.to_string()
        );
    }
}
