//! ## Ephemeral Tick Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

use crate::prelude::*;
use alloc::vec::Vec;
use alloy::{eips::BlockId, network::Network, providers::Provider};
use alloy_primitives::{aliases::I24, Address};
use derive_more::Deref;
use uniswap_lens::pool_lens;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, PartialEq, Deref)]
pub struct EphemeralTickDataProvider<I = I24> {
    pub pool: Address,
    pub tick_lower: I,
    pub tick_upper: I,
    pub tick_spacing: I,
    pub block_id: Option<BlockId>,
    #[deref]
    pub ticks: Vec<Tick<I>>,
}

impl<I: TickIndex> EphemeralTickDataProvider<I> {
    #[inline]
    pub async fn new<N, P>(
        pool: Address,
        provider: P,
        tick_lower: Option<I>,
        tick_upper: Option<I>,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        N: Network,
        P: Provider<N>,
    {
        let tick_lower = tick_lower.map_or(MIN_TICK, I::to_i24);
        let tick_upper = tick_upper.map_or(MAX_TICK, I::to_i24);
        let (ticks, tick_spacing) = pool_lens::get_populated_ticks_in_range(
            pool, tick_lower, tick_upper, provider, block_id,
        )
        .await
        .map_err(Error::LensError)?;
        let ticks: Vec<_> = ticks
            .into_iter()
            .map(|tick| {
                Tick::new(
                    I::from_i24(tick.tick),
                    tick.liquidityGross,
                    tick.liquidityNet,
                )
            })
            .collect();
        Ok(Self {
            pool,
            tick_lower: I::from_i24(tick_lower),
            tick_upper: I::from_i24(tick_upper),
            tick_spacing: I::from_i24(tick_spacing),
            block_id,
            ticks,
        })
    }
}

impl<I: TickIndex> From<EphemeralTickDataProvider<I>> for TickListDataProvider<I> {
    #[inline]
    fn from(provider: EphemeralTickDataProvider<I>) -> Self {
        assert!(!provider.ticks.is_empty());
        Self::new(provider.ticks, provider.tick_spacing)
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
            BLOCK_ID,
        )
        .await?;
        assert!(!provider.ticks.is_empty());
        provider.ticks.validate_list(TICK_SPACING);

        let tick = provider.get_tick(-92110).await?;
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(MIN_TICK_I32 + TICK_SPACING, true, TICK_SPACING)
            .await?;
        assert!(initialized);
        assert_eq!(tick, -887270);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(0, false, TICK_SPACING)
            .await?;
        assert!(initialized);
        assert_eq!(tick, 100);

        let provider: TickListDataProvider = provider.into();
        let tick = provider.get_tick(-92110).await?;
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);
        Ok(())
    }
}
