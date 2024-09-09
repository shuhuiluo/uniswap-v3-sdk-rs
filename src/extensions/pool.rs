//! ## Pool Extension
//! This module provides functions to create a [`Pool`] struct from a pool key and to fetch the
//! liquidity map within a tick range for the specified pool using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
//! in a single `eth_call`.

use crate::prelude::*;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    providers::Provider,
    transports::Transport,
};
use alloy_primitives::{Address, ChainId, B256};
use anyhow::Result;
use uniswap_lens::prelude::{
    get_populated_ticks_in_range, ierc20metadata::IERC20Metadata,
    iuniswapv3pool::IUniswapV3Pool::IUniswapV3PoolInstance,
};
use uniswap_sdk_core::{prelude::Token, token};

pub fn get_pool_contract<T, P>(
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    provider: P,
) -> IUniswapV3PoolInstance<T, P>
where
    T: Transport + Clone,
    P: Provider<T>,
{
    IUniswapV3PoolInstance::new(
        compute_pool_address(factory, token_a, token_b, fee, None),
        provider,
    )
}

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
pub async fn get_pool<T, P>(
    chain_id: ChainId,
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    provider: P,
    block_id: Option<BlockId>,
) -> Result<Pool, Error>
where
    T: Transport + Clone,
    P: Provider<T> + Clone,
{
    let block_id = block_id.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
    let pool_contract = get_pool_contract(factory, token_a, token_b, fee, provider.clone());
    let token_a_contract = IERC20Metadata::new(token_a, provider.clone());
    let token_b_contract = IERC20Metadata::new(token_b, provider.clone());
    // TODO: use multicall
    let slot_0 = pool_contract.slot0().block(block_id).call().await?;
    let liquidity = pool_contract.liquidity().block(block_id).call().await?._0;
    let token_a_decimals = token_a_contract.decimals().block(block_id).call().await?._0;
    let token_a_name = token_a_contract.name().block(block_id).call().await?._0;
    let token_a_symbol = token_a_contract.symbol().block(block_id).call().await?._0;
    let token_b_decimals = token_b_contract.decimals().block(block_id).call().await?._0;
    let token_b_name = token_b_contract.name().block(block_id).call().await?._0;
    let token_b_symbol = token_b_contract.symbol().block(block_id).call().await?._0;
    let sqrt_price_x96 = slot_0.sqrtPriceX96;
    if sqrt_price_x96.is_zero() {
        panic!("Pool has been created but not yet initialized");
    }
    Pool::new(
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

/// Normalizes the specified tick range.
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

/// Reconstructs the liquidity array from the tick array and the current liquidity.
fn reconstruct_liquidity_array<I: TickIndex>(
    tick_array: Vec<(I, i128)>,
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

#[allow(clippy::too_long_first_doc_paragraph)]
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
pub async fn get_liquidity_array_for_pool<TP, T, P>(
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
    T: Transport + Clone,
    P: Provider<T>,
{
    let (tick_current_aligned, tick_lower, tick_upper) = normalize_ticks(
        pool.tick_current,
        pool.tick_spacing(),
        tick_lower,
        tick_upper,
    );
    let ticks = get_populated_ticks_in_range(
        pool.address(init_code_hash_manual_override, factory_address_override),
        tick_lower.to_i24(),
        tick_upper.to_i24(),
        provider,
        block_id,
    )
    .await
    .map_err(|_| Error::LensError)?;
    reconstruct_liquidity_array(
        ticks
            .into_iter()
            .map(|tick| (TP::Index::from_i24(tick.tick), tick.liquidityNet))
            .collect(),
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
        get_pool(
            1,
            FACTORY_ADDRESS,
            address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            FeeAmount::LOW,
            PROVIDER.clone(),
            *BLOCK_ID,
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
            *BLOCK_ID,
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
