//! ## Simple Tick Data Provider
//! A data provider that fetches tick data from a Uniswap V3 pool contract on the fly.

use crate::prelude::*;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::{Ethereum, Network},
    providers::Provider,
    sol,
};
use alloy_primitives::{aliases::I24, Address, U256};

sol! {
    #[sol(rpc)]
    interface IUniswapV3PoolState {
        function ticks(int24 tick)
            external
            view
            returns (
                uint128 liquidityGross,
                int128 liquidityNet,
                uint256 feeGrowthOutside0X128,
                uint256 feeGrowthOutside1X128,
                int56 tickCumulativeOutside,
                uint160 secondsPerLiquidityOutsideX128,
                uint32 secondsOutside,
                bool initialized
            );
        function tickBitmap(int16 wordPosition) external view returns (uint256);
    }
}

/// A data provider that fetches tick data from a Uniswap V3 pool contract on the fly.
#[derive(Clone, Debug)]
pub struct SimpleTickDataProvider<P, N = Ethereum, I = I24>
where
    N: Network,
    P: Provider<N>,
    I: TickIndex,
{
    pub pool: IUniswapV3PoolState::IUniswapV3PoolStateInstance<P, N>,
    pub block_id: Option<BlockId>,
    _tick_index: core::marker::PhantomData<I>,
    _network: core::marker::PhantomData<N>,
}

impl<P, N, I> SimpleTickDataProvider<P, N, I>
where
    N: Network,
    P: Provider<N>,
    I: TickIndex,
{
    #[inline]
    pub const fn new(pool: Address, provider: P, block_id: Option<BlockId>) -> Self {
        Self {
            pool: IUniswapV3PoolState::new(pool, provider),
            block_id,
            _tick_index: core::marker::PhantomData,
            _network: core::marker::PhantomData,
        }
    }

    #[inline]
    pub const fn block_id(mut self, block_id: Option<BlockId>) -> Self {
        self.block_id = block_id;
        self
    }
}

impl<P, N, I> TickBitMapProvider for SimpleTickDataProvider<P, N, I>
where
    N: Network,
    P: Provider<N>,
    I: TickIndex,
{
    type Index = I;

    #[inline]
    async fn get_word(&self, index: Self::Index) -> Result<U256, Error> {
        let block_id = self
            .block_id
            .unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
        let word = self
            .pool
            .tickBitmap(index.to_i24().as_i16())
            .block(block_id)
            .call()
            .await?;
        Ok(U256::from(word))
    }
}

impl<P, N, I> TickDataProvider for SimpleTickDataProvider<P, N, I>
where
    N: Network,
    P: Provider<N>,
    I: TickIndex,
{
    type Index = I;

    #[inline]
    async fn get_tick(&self, index: Self::Index) -> Result<Tick<Self::Index>, Error> {
        let block_id = self
            .block_id
            .unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
        let tick = self
            .pool
            .ticks(index.to_i24())
            .block(block_id)
            .call()
            .await?;
        Ok(Tick {
            index,
            liquidity_gross: tick.liquidityGross,
            liquidity_net: tick.liquidityNet,
        })
    }

    #[inline]
    async fn next_initialized_tick_within_one_word(
        &self,
        tick: Self::Index,
        lte: bool,
        tick_spacing: Self::Index,
    ) -> Result<(Self::Index, bool), Error> {
        TickBitMapProvider::next_initialized_tick_within_one_word(self, tick, lte, tick_spacing)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::address;

    const TICK_SPACING: i32 = 10;

    #[tokio::test]
    async fn test_simple_tick_data_provider() -> Result<(), Error> {
        let provider = SimpleTickDataProvider::new(
            address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            PROVIDER.clone(),
            BLOCK_ID,
        );
        // [-887270, -92110, 100, 110, 22990, ...]
        let tick = provider.get_tick(-92110).await?;
        assert_eq!(tick.index, -92110);
        assert_eq!(tick.liquidity_gross, 398290794261);
        assert_eq!(tick.liquidity_net, 398290794261);

        let (tick, initialized) = TickDataProvider::next_initialized_tick_within_one_word(
            &provider,
            MIN_TICK_I32 + TICK_SPACING,
            true,
            TICK_SPACING,
        )
        .await?;
        assert_eq!(tick, -887270);
        assert!(initialized);

        let (tick, initialized) = TickDataProvider::next_initialized_tick_within_one_word(
            &provider,
            -92120,
            true,
            TICK_SPACING,
        )
        .await?;
        assert_eq!(tick, -92160);
        assert!(!initialized);

        let (tick, initialized) = TickDataProvider::next_initialized_tick_within_one_word(
            &provider,
            0,
            false,
            TICK_SPACING,
        )
        .await?;
        assert_eq!(tick, 100);
        assert!(initialized);

        let (tick, initialized) = TickDataProvider::next_initialized_tick_within_one_word(
            &provider,
            110,
            false,
            TICK_SPACING,
        )
        .await?;
        assert_eq!(tick, 2550);
        assert!(!initialized);
        Ok(())
    }
}
