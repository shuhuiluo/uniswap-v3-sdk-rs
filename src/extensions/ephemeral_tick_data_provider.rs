//! ## Ephemeral Tick Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

use crate::prelude::*;
use alloy::{eips::BlockId, providers::Provider, transports::Transport};
use alloy_primitives::{aliases::I24, Address};
use anyhow::Result;
use uniswap_lens::prelude::get_populated_ticks_in_range;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralTickDataProvider {
    pub pool: Address,
    pub tick_lower: I24,
    pub tick_upper: I24,
    pub block_id: Option<BlockId>,
    pub ticks: Vec<Tick>,
    /// the minimum distance between two ticks in the list
    pub tick_spacing: I24,
}

impl EphemeralTickDataProvider {
    pub async fn new<T, P>(
        pool: Address,
        provider: P,
        tick_lower: Option<I24>,
        tick_upper: Option<I24>,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        T: Transport + Clone,
        P: Provider<T>,
    {
        let tick_lower = tick_lower.unwrap_or(MIN_TICK);
        let tick_upper = tick_upper.unwrap_or(MAX_TICK);
        let ticks = get_populated_ticks_in_range(pool, tick_lower, tick_upper, provider, block_id)
            .await
            .map_err(|_| Error::LensError)?;
        let ticks: Vec<_> = ticks
            .into_iter()
            .map(|tick| Tick::new(tick.tick.as_i32(), tick.liquidityGross, tick.liquidityNet))
            .collect();
        let tick_indices: Vec<_> = ticks.iter().map(|tick| tick.index).collect();
        let tick_spacing: I24 = tick_indices
            .windows(2)
            .map(|window| window[1] - window[0])
            .min()
            .unwrap()
            .try_into()
            .unwrap();
        Ok(Self {
            pool,
            tick_lower,
            tick_upper,
            block_id,
            ticks,
            tick_spacing,
        })
    }
}

impl TickDataProvider for EphemeralTickDataProvider {
    type Tick = Tick;

    fn get_tick(&self, tick: i32) -> Result<&Tick, Error> {
        Ok(self.ticks.get_tick(tick))
    }

    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> Result<(i32, bool), Error> {
        Ok(self
            .ticks
            .next_initialized_tick_within_one_word(tick, lte, tick_spacing))
    }
}

impl From<EphemeralTickDataProvider> for TickListDataProvider {
    fn from(provider: EphemeralTickDataProvider) -> Self {
        assert!(!provider.ticks.is_empty());
        Self::new(provider.ticks, provider.tick_spacing.as_i32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::address;

    const TICK_SPACING: i32 = 10;

    #[tokio::test]
    async fn test_ephemeral_tick_data_provider() -> Result<(), Error> {
        let provider = EphemeralTickDataProvider::new(
            address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            PROVIDER.clone(),
            None,
            None,
            Some(BlockId::from(17000000)),
        )
        .await?;
        assert!(!provider.ticks.is_empty());
        provider.ticks.validate_list(TICK_SPACING);
        let tick = provider.get_tick(-92110)?;
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);
        let (tick, success) = provider.next_initialized_tick_within_one_word(
            MIN_TICK.as_i32() + TICK_SPACING,
            true,
            TICK_SPACING,
        )?;
        assert!(success);
        assert_eq!(tick, -887270);
        let (tick, success) =
            provider.next_initialized_tick_within_one_word(0, false, TICK_SPACING)?;
        assert!(success);
        assert_eq!(tick, 100);
        let provider: TickListDataProvider = provider.into();
        let tick = provider.get_tick(-92110)?;
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);
        Ok(())
    }
}
