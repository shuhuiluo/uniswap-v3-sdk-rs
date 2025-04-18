use crate::prelude::*;
use core::ops::Deref;

/// Provides information about ticks
pub trait TickDataProvider: Send + Sync {
    type Index: TickIndex;

    /// Return information corresponding to a specific tick
    ///
    /// ## Arguments
    ///
    /// * `index`: The tick to load
    ///
    /// returns: Result<Tick<Self::Index>, Error>
    async fn get_tick(&self, index: Self::Index) -> Result<Tick<Self::Index>, Error>;

    /// Return the next tick that is initialized within a single word
    ///
    /// ## Arguments
    ///
    /// * `tick`: The current tick
    /// * `lte`: Whether the next tick should be lte the current tick
    /// * `tick_spacing`: The tick spacing of the pool
    ///
    /// returns: Result<(Self::Index, bool), Error>
    async fn next_initialized_tick_within_one_word(
        &self,
        tick: Self::Index,
        lte: bool,
        tick_spacing: Self::Index,
    ) -> Result<(Self::Index, bool), Error>;
}

/// Implements the [`TickDataProvider`] trait for any type that dereferences to a
/// [`TickDataProvider`]
impl<TP> TickDataProvider for TP
where
    TP: Deref<Target: TickDataProvider> + Send + Sync,
{
    type Index = <<TP as Deref>::Target as TickDataProvider>::Index;

    #[inline]
    async fn get_tick(&self, index: Self::Index) -> Result<Tick<Self::Index>, Error> {
        self.deref().get_tick(index).await
    }

    #[inline]
    async fn next_initialized_tick_within_one_word(
        &self,
        tick: Self::Index,
        lte: bool,
        tick_spacing: Self::Index,
    ) -> Result<(Self::Index, bool), Error> {
        self.deref()
            .next_initialized_tick_within_one_word(tick, lte, tick_spacing)
            .await
    }
}

/// This tick data provider does not know how to fetch any tick data. It throws whenever it is
/// required. Useful if you do not need to load tick data for your use case.
#[derive(Clone, Copy, Debug)]
pub struct NoTickDataProvider;

impl TickDataProvider for NoTickDataProvider {
    type Index = i32;

    #[inline]
    async fn get_tick(&self, _: i32) -> Result<Tick, Error> {
        Err(Error::NoTickDataError)
    }

    #[inline]
    async fn next_initialized_tick_within_one_word(
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

    #[tokio::test]
    async fn test_no_tick_data_provider() {
        let tick_data_provider = NoTickDataProvider;
        assert_eq!(
            tick_data_provider
                .get_tick(0)
                .await
                .unwrap_err()
                .to_string(),
            Error::NoTickDataError.to_string()
        );
        assert_eq!(
            tick_data_provider
                .next_initialized_tick_within_one_word(0, false, 1)
                .await
                .unwrap_err()
                .to_string(),
            Error::NoTickDataError.to_string()
        );
    }
}
