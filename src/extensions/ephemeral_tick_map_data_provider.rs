//! ## Ephemeral Tick Map Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

#![allow(unused_variables)]
use crate::prelude::*;
use alloy::{eips::BlockId, providers::Provider, transports::Transport};
use alloy_primitives::{aliases::I24, Address};
use uniswap_lens::pool_lens;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralTickMapDataProvider<I = I24> {
    pub pool: Address,
    pub tick_lower: I,
    pub tick_upper: I,
    pub block_id: Option<BlockId>,
    pub tick_map: TickMap,
    /// the minimum distance between two ticks in the list
    pub tick_spacing: I,
}

impl<I: TickIndex> EphemeralTickMapDataProvider<I> {
    #[inline]
    pub async fn new<T, P>(
        pool: Address,
        provider: P,
        tick_lower: Option<I>,
        tick_upper: Option<I>,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        T: Transport + Clone,
        P: Provider<T>,
    {
        let tick_lower = tick_lower.map_or(MIN_TICK, I::to_i24);
        let tick_upper = tick_upper.map_or(MAX_TICK, I::to_i24);
        let (ticks, tick_spacing) = pool_lens::get_populated_ticks_in_range(
            pool, tick_lower, tick_upper, provider, block_id,
        )
        .await
        .map_err(Error::LensError)?;
        unimplemented!()
    }
}

impl<I: TickIndex> TickDataProvider for EphemeralTickMapDataProvider<I> {
    type Index = I;

    #[inline]
    fn get_tick(&self, tick: I) -> Result<&Tick<I>, Error> {
        unimplemented!()
    }

    #[inline]
    fn next_initialized_tick_within_one_word(
        &self,
        tick: I,
        lte: bool,
        tick_spacing: I,
    ) -> Result<(I, bool), Error> {
        unimplemented!()
    }
}
