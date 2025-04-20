//! ## Ephemeral Tick Map Data Provider
//! A data provider that fetches ticks using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol) in a single `eth_call`.

use crate::prelude::*;
use alloy::{eips::BlockId, network::Network, providers::Provider};
use alloy_primitives::{aliases::I24, Address};
use derive_more::Deref;

/// A data provider that fetches ticks using an ephemeral contract in a single `eth_call`.
#[derive(Clone, Debug, Deref)]
pub struct EphemeralTickMapDataProvider<I = I24> {
    pub pool: Address,
    pub tick_lower: I,
    pub tick_upper: I,
    pub tick_spacing: I,
    pub block_id: Option<BlockId>,
    #[deref]
    pub tick_map: TickMap<I>,
}

impl<I: TickIndex> EphemeralTickMapDataProvider<I> {
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
        let provider =
            EphemeralTickDataProvider::new(pool, provider, tick_lower, tick_upper, block_id)
                .await?;
        Ok(Self {
            pool,
            tick_lower: provider.tick_lower,
            tick_upper: provider.tick_upper,
            tick_spacing: provider.tick_spacing,
            block_id,
            tick_map: TickMap::new(provider.ticks, provider.tick_spacing),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::address;

    const TICK_SPACING: i32 = 10;

    #[tokio::test]
    async fn test_ephemeral_tick_map_data_provider() -> Result<(), Error> {
        let provider = EphemeralTickMapDataProvider::new(
            address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            PROVIDER.clone(),
            None,
            None,
            BLOCK_ID,
        )
        .await?;
        // [-887270, -92110, 100, 110, 22990, ...]
        let tick = provider.get_tick(-92110).await?;
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(MIN_TICK_I32 + TICK_SPACING, true, TICK_SPACING)
            .await?;
        assert_eq!(tick, -887270);
        assert!(initialized);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(-92120, true, TICK_SPACING)
            .await?;
        assert_eq!(tick, -92160);
        assert!(!initialized);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(0, false, TICK_SPACING)
            .await?;
        assert_eq!(tick, 100);
        assert!(initialized);

        let (tick, initialized) = provider
            .next_initialized_tick_within_one_word(110, false, TICK_SPACING)
            .await?;
        assert_eq!(tick, 2550);
        assert!(!initialized);
        Ok(())
    }
}
