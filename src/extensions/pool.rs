//! ## Pool Extension
//! This module provides functions to create a [`Pool`] struct from a pool key and to fetch the
//! liquidity map within a tick range for the specified pool using an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
//! in a single `eth_call`.

use crate::prelude::*;
use alloy_primitives::{Address, ChainId, B256};
use anyhow::Result;
use aperture_lens::prelude::{
    get_populated_ticks_in_range,
    i_uniswap_v3_pool::{IUniswapV3Pool, Slot0Return},
    ierc20_metadata::IERC20Metadata,
};
use ethers::prelude::*;
use num_integer::Integer;
use std::sync::Arc;
use uniswap_sdk_core::{prelude::Token, token};

pub fn get_pool_contract<M: Middleware>(
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    client: Arc<M>,
) -> IUniswapV3Pool<M> {
    IUniswapV3Pool::new(
        compute_pool_address(factory, token_a, token_b, fee, None).into_array(),
        client,
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
/// * `client`: The client
/// * `block_id`: Optional block number to query.
pub async fn get_pool<M: Middleware>(
    chain_id: ChainId,
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    client: Arc<M>,
    block_id: Option<BlockId>,
) -> Result<Pool<NoTickDataProvider>, MulticallError<M>> {
    let pool_contract = get_pool_contract(factory, token_a, token_b, fee, client.clone());
    let token_a_contract = IERC20Metadata::new(token_a.into_array(), client.clone());
    let token_b_contract = IERC20Metadata::new(token_b.into_array(), client.clone());
    let mut multicall = Multicall::new_with_chain_id(client, None, Some(chain_id)).unwrap();
    multicall.block = block_id;
    multicall
        .add_call(pool_contract.slot_0(), false)
        .add_call(pool_contract.liquidity(), false)
        .add_call(token_a_contract.decimals(), false)
        .add_call(token_a_contract.name(), false)
        .add_call(token_a_contract.symbol(), false)
        .add_call(token_b_contract.decimals(), false)
        .add_call(token_b_contract.name(), false)
        .add_call(token_b_contract.symbol(), false);
    let (
        slot_0,
        liquidity,
        token_a_decimals,
        token_a_name,
        token_a_symbol,
        token_b_decimals,
        token_b_name,
        token_b_symbol,
    ): (Slot0Return, u128, u8, String, String, u8, String, String) = multicall.call().await?;
    let sqrt_price_x96 = slot_0.sqrt_price_x96;
    if sqrt_price_x96.is_zero() {
        panic!("Pool has been created but not yet initialized");
    }
    Ok(Pool::new(
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
        sqrt_price_x96.to_alloy(),
        liquidity,
    )
    .unwrap())
}

/// Normalizes the specified tick range.
fn normalize_ticks(
    tick_current: i32,
    tick_spacing: i32,
    tick_lower: i32,
    tick_upper: i32,
) -> (i32, i32, i32) {
    assert!(tick_lower <= tick_upper, "tickLower > tickUpper");
    // The current tick must be within the specified tick range.
    let tick_current_aligned = tick_current.div_mod_floor(&tick_spacing).0 * tick_spacing;
    let tick_lower = tick_lower.max(MIN_TICK).min(tick_current_aligned);
    let tick_upper = tick_upper.min(MAX_TICK).max(tick_current_aligned);
    (tick_current_aligned, tick_lower, tick_upper)
}

/// Reconstructs the liquidity array from the tick array and the current liquidity.
fn reconstruct_liquidity_array(
    tick_array: Vec<(i32, i128)>,
    tick_current_aligned: i32,
    current_liquidity: u128,
) -> Result<Vec<(i32, u128)>> {
    // Locate the tick in the populated ticks array with the current liquidity.
    let current_index = tick_array
        .iter()
        .position(|&(tick, _)| tick > tick_current_aligned)
        .unwrap()
        - 1;
    // Accumulate the liquidity from the current tick to the end of the populated ticks array.
    let mut cumulative_liquidity = current_liquidity;
    let mut liquidity_array = vec![(0, 0); tick_array.len()];
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
/// * `client`: The client.
/// * `block_id`: Optional block number to query.
/// * `init_code_hash_manual_override`: Optional init code hash override.
/// * `factory_address_override`: Optional factory address override.
///
/// ## Returns
///
/// An array of ticks and corresponding cumulative liquidity.
pub async fn get_liquidity_array_for_pool<M: Middleware, P>(
    pool: Pool<P>,
    tick_lower: i32,
    tick_upper: i32,
    client: Arc<M>,
    block_id: Option<BlockId>,
    init_code_hash_manual_override: Option<B256>,
    factory_address_override: Option<Address>,
) -> Result<Vec<(i32, u128)>, ContractError<M>> {
    let (tick_current_aligned, tick_lower, tick_upper) = normalize_ticks(
        pool.tick_current,
        pool.tick_spacing(),
        tick_lower,
        tick_upper,
    );
    let ticks = get_populated_ticks_in_range(
        pool.address(init_code_hash_manual_override, factory_address_override)
            .to_ethers(),
        tick_lower,
        tick_upper,
        client,
        block_id,
    )
    .await?;
    Ok(reconstruct_liquidity_array(
        ticks
            .into_iter()
            .map(|tick| (tick.tick, tick.liquidity_net))
            .collect(),
        tick_current_aligned,
        pool.liquidity,
    )
    .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    async fn pool() -> Pool<NoTickDataProvider> {
        get_pool(
            1,
            FACTORY_ADDRESS,
            address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            FeeAmount::LOW,
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17000000)),
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
        let tick_current_aligned =
            pool.tick_current.div_mod_floor(&pool.tick_spacing()).0 * pool.tick_spacing();
        let liquidity = pool.liquidity;
        let tick_lower = pool.tick_current - DOUBLE_TICK;
        let tick_upper = pool.tick_current + DOUBLE_TICK;
        let liquidity_array = get_liquidity_array_for_pool(
            pool,
            tick_lower,
            tick_upper,
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17000000)),
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
