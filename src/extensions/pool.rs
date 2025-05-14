//! ## Pool Extension
//! This module provides functions to create a [`Pool`] struct from a pool key and to fetch the
//! liquidity map within a tick range for the specified pool using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
//! in a single `eth_call`.

use crate::prelude::*;
use alloc::{string::ToString, vec, vec::Vec};
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::Network,
    providers::Provider,
};
use alloy_primitives::{Address, ChainId, B256};
use uniswap_lens::{
    bindings::{
        ierc20metadata::IERC20Metadata, iuniswapv3pool::IUniswapV3Pool::IUniswapV3PoolInstance,
    },
    pool_lens,
};
use uniswap_sdk_core::{prelude::Token, token};

#[inline]
pub fn get_pool_contract<N, P>(
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    provider: P,
) -> IUniswapV3PoolInstance<P, N>
where
    N: Network,
    P: Provider<N>,
{
    IUniswapV3PoolInstance::new(
        compute_pool_address(factory, token_a, token_b, fee, None, None),
        provider,
    )
}

impl Pool {
    /// Get a [`Pool`] struct from pool key
    ///
    /// ## Arguments
    ///
    /// * `chain_id`: The chain id
    /// * `factory`: The factory address
    /// * `token_a`: One of the tokens in the pool
    /// * `token_b`: The other token in the pool
    /// * `fee`: Fee tier of the pool
    /// * `provider`: The alloy provider
    /// * `block_id`: Optional block number to query.
    #[inline]
    pub async fn from_pool_key<N, P>(
        chain_id: ChainId,
        factory: Address,
        token_a: Address,
        token_b: Address,
        fee: FeeAmount,
        provider: P,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        N: Network,
        P: Provider<N>,
    {
        let block_id = block_id.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
        let pool_contract = get_pool_contract(factory, token_a, token_b, fee, provider.root());
        let token_a_contract = IERC20Metadata::new(token_a, provider.root());
        let token_b_contract = IERC20Metadata::new(token_b, provider.root());
        let multicall = provider
            .multicall()
            .add(pool_contract.slot0())
            .add(pool_contract.liquidity())
            .add(token_a_contract.decimals())
            .add(token_a_contract.name())
            .add(token_a_contract.symbol())
            .add(token_b_contract.decimals())
            .add(token_b_contract.name())
            .add(token_b_contract.symbol());
        let (
            slot_0,
            liquidity,
            token_a_decimals,
            token_a_name,
            token_a_symbol,
            token_b_decimals,
            token_b_name,
            token_b_symbol,
        ) = multicall.block(block_id).aggregate().await?;
        let sqrt_price_x96 = slot_0.sqrtPriceX96;
        assert!(
            !sqrt_price_x96.is_zero(),
            "Pool has been created but not yet initialized"
        );
        Self::new(
            token!(
                chain_id,
                token_a,
                token_a_decimals,
                token_a_symbol,
                token_a_name
            ),
            token!(
                chain_id,
                token_b,
                token_b_decimals,
                token_b_symbol,
                token_b_name
            ),
            fee,
            sqrt_price_x96,
            liquidity,
        )
    }
}

impl<I: TickIndex> Pool<EphemeralTickMapDataProvider<I>> {
    /// Get a [`Pool`] struct with tick data provider from pool key
    ///
    /// ## Arguments
    ///
    /// * `chain_id`: The chain id
    /// * `factory`: The factory address
    /// * `token_a`: One of the tokens in the pool
    /// * `token_b`: The other token in the pool
    /// * `fee`: Fee tier of the pool
    /// * `provider`: The alloy provider
    /// * `block_id`: Optional block number to query.
    ///
    /// ## Returns
    ///
    /// A [`Pool`] struct with tick data provider
    ///
    /// ## Examples
    ///
    /// ```
    /// use alloy::{eips::BlockId, providers::ProviderBuilder};
    /// use alloy_primitives::address;
    /// use uniswap_v3_sdk::prelude::*;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     dotenv::dotenv().ok();
    ///     let rpc_url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    ///     let provider = ProviderBuilder::new().connect_http(rpc_url);
    ///     let block_id = Some(BlockId::from(17000000));
    ///     let pool = Pool::<EphemeralTickMapDataProvider>::from_pool_key_with_tick_data_provider(
    ///         1,
    ///         FACTORY_ADDRESS,
    ///         address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
    ///         address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
    ///         FeeAmount::LOW,
    ///         provider,
    ///         block_id,
    ///     )
    ///     .await
    ///     .unwrap();
    /// }
    /// ```
    #[inline]
    pub async fn from_pool_key_with_tick_data_provider<N, P>(
        chain_id: ChainId,
        factory: Address,
        token_a: Address,
        token_b: Address,
        fee: FeeAmount,
        provider: P,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        N: Network,
        P: Provider<N>,
    {
        let pool = Pool::from_pool_key(
            chain_id,
            factory,
            token_a,
            token_b,
            fee,
            provider.root(),
            block_id,
        )
        .await?;
        let tick_data_provider = EphemeralTickMapDataProvider::new(
            pool.address(None, Some(factory)),
            provider,
            None,
            None,
            block_id,
        )
        .await?;
        Self::new_with_tick_data_provider(
            pool.token0,
            pool.token1,
            pool.fee,
            pool.sqrt_ratio_x96,
            pool.liquidity,
            tick_data_provider,
        )
    }
}

/// Normalizes the specified tick range.
#[inline]
fn normalize_ticks<I: TickIndex>(
    tick_current: I,
    tick_spacing: I,
    tick_lower: I,
    tick_upper: I,
) -> (I, I, I) {
    assert!(tick_lower <= tick_upper, "tickLower > tickUpper");
    // The current tick must be within the specified tick range.
    let tick_current_aligned = tick_current.div(tick_spacing) * tick_spacing;
    let tick_lower = tick_lower
        .max(I::from_i24(MIN_TICK))
        .min(tick_current_aligned);
    let tick_upper = tick_upper
        .min(I::from_i24(MAX_TICK))
        .max(tick_current_aligned);
    (tick_current_aligned, tick_lower, tick_upper)
}

/// Reconstructs the liquidity array from the tick array and the current liquidity
///
/// ## Arguments
///
/// * `tick_array`: The tick array of tick and net liquidity sorted by tick
/// * `tick_current_aligned`: The current tick aligned to the tick spacing
/// * `current_liquidity`: The current liquidity
///
/// ## Returns
///
/// An array of ticks and corresponding cumulative liquidity
#[inline]
pub fn reconstruct_liquidity_array<I: TickIndex>(
    tick_array: &[(I, i128)],
    tick_current_aligned: I,
    current_liquidity: u128,
) -> Result<Vec<(I, u128)>, Error> {
    // Locate the tick in the populated ticks array with the current liquidity.
    let current_index = tick_array
        .iter()
        .position(|&(tick, _)| tick > tick_current_aligned)
        .unwrap()
        - 1;
    // Accumulate the liquidity from the current tick to the end of the populated ticks array.
    let mut cumulative_liquidity = current_liquidity;
    let mut liquidity_array = vec![(I::ZERO, 0); tick_array.len()];
    for (i, &(tick, liquidity_net)) in tick_array.iter().enumerate().skip(current_index + 1) {
        // added when tick is crossed from left to right
        cumulative_liquidity = add_delta(cumulative_liquidity, liquidity_net)?;
        liquidity_array[i] = (tick, cumulative_liquidity);
    }
    cumulative_liquidity = current_liquidity;
    for (i, &(tick, liquidity_net)) in tick_array.iter().enumerate().take(current_index + 1).rev() {
        liquidity_array[i] = (tick, cumulative_liquidity);
        // subtracted when tick is crossed from right to left
        cumulative_liquidity = add_delta(cumulative_liquidity, -liquidity_net)?;
    }
    Ok(liquidity_array)
}

/// Fetches the liquidity within a tick range for the specified pool, using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
/// in a single `eth_call`.
///
/// ## Arguments
///
/// * `pool`: The liquidity pool to fetch the tick to liquidity map for.
/// * `tick_lower`: The lower tick to fetch liquidity for.
/// * `tick_upper`: The upper tick to fetch liquidity for.
/// * `provider`: The alloy provider.
/// * `block_id`: Optional block number to query.
/// * `init_code_hash_manual_override`: Optional init code hash override.
/// * `factory_address_override`: Optional factory address override.
///
/// ## Returns
///
/// An array of ticks and corresponding cumulative liquidity.
#[inline]
pub async fn get_liquidity_array_for_pool<TP, N, P>(
    pool: Pool<TP>,
    tick_lower: TP::Index,
    tick_upper: TP::Index,
    provider: P,
    block_id: Option<BlockId>,
    init_code_hash_manual_override: Option<B256>,
    factory_address_override: Option<Address>,
) -> Result<Vec<(TP::Index, u128)>, Error>
where
    TP: TickDataProvider,
    N: Network,
    P: Provider<N>,
{
    let (tick_current_aligned, tick_lower, tick_upper) = normalize_ticks(
        pool.tick_current,
        pool.tick_spacing(),
        tick_lower,
        tick_upper,
    );
    let (ticks, _) = pool_lens::get_populated_ticks_in_range(
        pool.address(init_code_hash_manual_override, factory_address_override),
        tick_lower.to_i24(),
        tick_upper.to_i24(),
        provider,
        block_id,
    )
    .await
    .map_err(Error::LensError)?;
    reconstruct_liquidity_array(
        &ticks
            .into_iter()
            .map(|tick| (TP::Index::from_i24(tick.tick), tick.liquidityNet))
            .collect::<Vec<(TP::Index, i128)>>(),
        tick_current_aligned,
        pool.liquidity,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::address;

    async fn pool() -> Pool {
        Pool::from_pool_key(
            1,
            FACTORY_ADDRESS,
            address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            FeeAmount::LOW,
            PROVIDER.clone(),
            BLOCK_ID,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_get_pool() {
        let pool = pool().await;
        assert_eq!(pool.token0.symbol.unwrap(), "WBTC");
        assert_eq!(pool.token1.symbol.unwrap(), "WETH");
        assert_eq!(pool.tick_current, 257344);
        assert_eq!(pool.liquidity, 786352807736110014);
    }

    #[tokio::test]
    async fn test_get_liquidity_array_for_pool() {
        let pool = pool().await;
        const DOUBLE_TICK: i32 = 6932;
        let tick_current_aligned = pool.tick_current / pool.tick_spacing() * pool.tick_spacing();
        let liquidity = pool.liquidity;
        let tick_lower = pool.tick_current - DOUBLE_TICK;
        let tick_upper = pool.tick_current + DOUBLE_TICK;
        let liquidity_array = get_liquidity_array_for_pool(
            pool,
            tick_lower,
            tick_upper,
            PROVIDER.clone(),
            BLOCK_ID,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(!liquidity_array.is_empty());
        assert_eq!(
            liquidity_array[liquidity_array
                .iter()
                .position(|&(tick, _)| tick > tick_current_aligned)
                .unwrap()
                - 1]
            .1,
            liquidity
        );
    }
}
