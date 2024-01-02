use crate::{
    entities::{Tick, TickDataProvider},
    utils::{TickList, MAX_TICK, MIN_TICK},
};
use alloy_primitives::Address;
use anyhow::Result;
use aperture_lens::prelude::get_populated_ticks_in_range;
use ethers::prelude::{BlockId, ContractError, Middleware};
use std::sync::Arc;

/// A data provider for ticks that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone)]
pub struct EphemeralTickDataProvider<M: Middleware> {
    pub pool: Address,
    client: Arc<M>,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub block_id: Option<BlockId>,
    pub ticks: Vec<Tick>,
}

impl<M: Middleware> EphemeralTickDataProvider<M> {
    pub fn new(
        pool: Address,
        client: Arc<M>,
        tick_lower: Option<i32>,
        tick_upper: Option<i32>,
        block_id: Option<BlockId>,
    ) -> Self {
        Self {
            pool,
            tick_lower: tick_lower.unwrap_or(MIN_TICK),
            tick_upper: tick_upper.unwrap_or(MAX_TICK),
            client,
            block_id,
            ticks: Vec::new(),
        }
    }

    pub async fn fetch(&mut self) -> Result<(), ContractError<M>> {
        let ticks = get_populated_ticks_in_range(
            self.pool.into_array().into(),
            self.tick_lower,
            self.tick_upper,
            self.client.clone(),
            self.block_id,
        )
        .await?;
        self.ticks = ticks
            .into_iter()
            .map(|tick| Tick::new(tick.tick, tick.liquidity_gross, tick.liquidity_net))
            .collect();
        Ok(())
    }
}

impl<M: Middleware> TickDataProvider<Tick> for EphemeralTickDataProvider<M> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use ethers::prelude::{Http, Provider, MAINNET};
    use once_cell::sync::Lazy;

    static PROVIDER: Lazy<EphemeralTickDataProvider<Provider<Http>>> = Lazy::new(|| {
        let provider = Arc::new(MAINNET.provider());
        EphemeralTickDataProvider::new(
            address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            provider,
            None,
            None,
            Some(BlockId::from(17000000)),
        )
    });
    const TICK_SPACING: i32 = 10;

    #[tokio::test]
    async fn test_ephemeral_tick_data_provider() -> Result<()> {
        let mut provider = PROVIDER.clone();
        provider.fetch().await?;
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
        Ok(())
    }
}
