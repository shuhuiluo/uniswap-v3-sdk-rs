//! ## Ephemeral Tick Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

use crate::prelude::*;
use alloy_primitives::Address;
use anyhow::Result;
use aperture_lens::prelude::get_populated_ticks_in_range;
use ethers::prelude::{BlockId, ContractError, Middleware};
use std::sync::Arc;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralTickDataProvider {
    pub pool: Address,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub block_id: Option<BlockId>,
    pub ticks: Vec<Tick>,
    /// the minimum distance between two ticks in the list
    pub tick_spacing: i32,
}

impl EphemeralTickDataProvider {
    pub async fn new<M: Middleware>(
        pool: Address,
        client: Arc<M>,
        tick_lower: Option<i32>,
        tick_upper: Option<i32>,
        block_id: Option<BlockId>,
    ) -> Result<Self, ContractError<M>> {
        let tick_lower = tick_lower.unwrap_or(MIN_TICK);
        let tick_upper = tick_upper.unwrap_or(MAX_TICK);
        let ticks = get_populated_ticks_in_range(
            pool.to_ethers(),
            tick_lower,
            tick_upper,
            client.clone(),
            block_id,
        )
        .await?;
        let ticks: Vec<_> = ticks
            .into_iter()
            .map(|tick| Tick::new(tick.tick, tick.liquidity_gross, tick.liquidity_net))
            .collect();
        let tick_indices: Vec<_> = ticks.iter().map(|tick| tick.index).collect();
        let tick_spacing = tick_indices
            .windows(2)
            .map(|window| window[1] - window[0])
            .min()
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

    fn get_tick(&self, tick: i32) -> Result<&Tick> {
        Ok(self.ticks.get_tick(tick))
    }

    fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        lte: bool,
        tick_spacing: i32,
    ) -> Result<(i32, bool)> {
        Ok(self
            .ticks
            .next_initialized_tick_within_one_word(tick, lte, tick_spacing))
    }
}

impl From<EphemeralTickDataProvider> for TickListDataProvider {
    fn from(provider: EphemeralTickDataProvider) -> Self {
        assert!(!provider.ticks.is_empty());
        Self::new(provider.ticks, provider.tick_spacing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use ethers::prelude::{Provider, MAINNET};

    const TICK_SPACING: i32 = 10;

    #[tokio::test]
    #[ignore] // for flakiness
    async fn test_ephemeral_tick_data_provider() -> Result<()> {
        let provider = EphemeralTickDataProvider::new(
            address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            Arc::new(Provider::new_client(MAINNET.provider().url().as_str(), 3, 1000).unwrap()),
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
            MIN_TICK + TICK_SPACING,
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
