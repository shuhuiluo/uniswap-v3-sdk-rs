//! ## Ephemeral Tick Map Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

#![allow(unused_variables)]
use crate::prelude::*;
use alloy::{eips::BlockId, providers::Provider, transports::Transport};
use alloy_primitives::{aliases::I24, Address};
use anyhow::Result;
use uniswap_lens::prelude::get_populated_ticks_in_range;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralTickMapDataProvider {
    pub pool: Address,
    pub tick_lower: I24,
    pub tick_upper: I24,
    pub block_id: Option<BlockId>,
    pub tick_map: TickMap,
    /// the minimum distance between two ticks in the list
    pub tick_spacing: I24,
}

impl EphemeralTickMapDataProvider {
    pub async fn new<T, P>(
        pool: Address,
        provider: P,
        tick_lower: Option<I24>,
        tick_upper: Option<I24>,
        block_id: Option<BlockId>,
    ) -> Result<Self>
    where
        T: Transport + Clone,
        P: Provider<T>,
    {
        let tick_lower = tick_lower.unwrap_or(MIN_TICK);
        let tick_upper = tick_upper.unwrap_or(MAX_TICK);
        let ticks =
            get_populated_ticks_in_range(pool, tick_lower, tick_upper, provider, block_id).await?;
        unimplemented!()
    }
}

impl TickDataProvider for EphemeralTickMapDataProvider {
    type Tick = Tick;

    fn get_tick(&self, tick: i32) -> Result<&Tick, Error> {
        unimplemented!()
    }

    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> Result<(i32, bool), Error> {
        unimplemented!()
    }
}
