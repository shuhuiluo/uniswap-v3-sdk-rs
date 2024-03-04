//! ## Position Extension
//! This module provides functions to create a [`Position`] struct from the token id, get the state
//! and pool for all positions of the specified owner by deploying an ephemeral contract via
//! `eth_call`, etc.

use crate::prelude::*;
use alloy_primitives::{Address, ChainId, U256};
use anyhow::Result;
use aperture_lens::{
    position_lens,
    prelude::{
        i_nonfungible_position_manager::{INonfungiblePositionManager, PositionsReturn},
        i_uniswap_v3_pool::{Slot0Return, TicksReturn},
        shared_types::PositionState,
    },
};
use base64::{engine::general_purpose, Engine};
use ethers::prelude::*;
use std::sync::Arc;
use uniswap_sdk_core::{prelude::*, token};

pub fn get_nonfungible_position_manager_contract<M: Middleware>(
    nonfungible_position_manager: Address,
    client: Arc<M>,
) -> INonfungiblePositionManager<M> {
    INonfungiblePositionManager::new(nonfungible_position_manager.into_array(), client)
}

/// Get a [`Position`] struct from the token id
///
/// ## Arguments
///
/// * `chain_id`: The chain id
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `token_id`: The token id
/// * `client`: The client
/// * `block_id`: Optional block number to query
pub async fn get_position<M: Middleware>(
    chain_id: ChainId,
    nonfungible_position_manager: Address,
    token_id: U256,
    client: Arc<M>,
    block_id: Option<BlockId>,
) -> Result<Position<NoTickDataProvider>, MulticallError<M>> {
    let npm_contract =
        get_nonfungible_position_manager_contract(nonfungible_position_manager, client.clone());
    let mut multicall = Multicall::new_with_chain_id(client.clone(), None, Some(chain_id)).unwrap();
    multicall.block = block_id;
    multicall
        .add_call(npm_contract.positions(token_id.to_ethers()), false)
        .add_call(npm_contract.factory(), false);
    let (position, factory): (PositionsReturn, types::Address) = multicall.call().await?;
    let PositionsReturn {
        token_0,
        token_1,
        fee,
        tick_lower,
        tick_upper,
        liquidity,
        ..
    } = position;
    let pool = get_pool(
        chain_id,
        factory.to_alloy(),
        token_0.to_alloy(),
        token_1.to_alloy(),
        fee.into(),
        client,
        block_id,
    )
    .await?;
    Ok(Position::new(pool, liquidity, tick_lower, tick_upper))
}

impl Position<NoTickDataProvider> {
    /// Get a [`Position`] struct from the token id in a single call by deploying an ephemeral
    /// contract via `eth_call`
    ///
    /// ## Arguments
    ///
    /// * `chain_id`: The chain id
    /// * `nonfungible_position_manager`: The nonfungible position manager address
    /// * `token_id`: The token id
    /// * `client`: The client
    /// * `block_id`: Optional block number to query
    pub async fn from_token_id<M: Middleware>(
        chain_id: ChainId,
        nonfungible_position_manager: Address,
        token_id: U256,
        client: Arc<M>,
        block_id: Option<BlockId>,
    ) -> Result<Self, ContractError<M>> {
        let PositionState {
            position,
            slot_0,
            active_liquidity,
            decimals_0,
            decimals_1,
            ..
        } = position_lens::get_position_details(
            nonfungible_position_manager.to_ethers(),
            token_id.to_ethers(),
            client,
            block_id,
        )
        .await?;
        let token_0: Address = position.token_0.to_alloy();
        let token_1: Address = position.token_1.to_alloy();
        let pool = Pool::new(
            token!(chain_id, token_0, decimals_0),
            token!(chain_id, token_1, decimals_1),
            position.fee.into(),
            slot_0.sqrt_price_x96.to_alloy(),
            active_liquidity,
        )
        .unwrap();
        Ok(Position::new(
            pool,
            position.liquidity,
            position.tick_lower,
            position.tick_upper,
        ))
    }
}

/// Get the state and pool for all positions of the specified owner by deploying an ephemeral
/// contract via `eth_call`.
///
/// ## Note
///
/// Each position consumes about 200k gas, so this method may fail if the number of positions
/// exceeds 1500 assuming the provider gas limit is 300m.
///
/// ## Arguments
///
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `owner`: The owner address
/// * `client`: The client
/// * `block_id`: Optional block number to query
pub async fn get_all_positions_by_owner<M: Middleware>(
    nonfungible_position_manager: Address,
    owner: Address,
    client: Arc<M>,
    block_id: Option<BlockId>,
) -> Result<Vec<PositionState>, ContractError<M>> {
    position_lens::get_all_positions_by_owner(
        nonfungible_position_manager.to_ethers(),
        owner.to_ethers(),
        client,
        block_id,
    )
    .await
}

/// Get the real-time collectable token amounts.
///
/// ## Arguments
///
/// * `chain_id`: The chain id
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `token_id`: The token id
/// * `client`: The client
/// * `block_id`: Optional block number to query
///
/// ## Returns
///
/// A tuple of the collectable token amounts.
pub async fn get_collectable_token_amounts<M: Middleware>(
    chain_id: ChainId,
    nonfungible_position_manager: Address,
    token_id: U256,
    client: Arc<M>,
    block_id: Option<BlockId>,
) -> Result<(U256, U256), MulticallError<M>> {
    let npm_contract =
        get_nonfungible_position_manager_contract(nonfungible_position_manager, client.clone());
    let mut multicall = Multicall::new_with_chain_id(client.clone(), None, Some(chain_id)).unwrap();
    multicall.block = block_id;
    multicall
        .add_call(npm_contract.positions(token_id.to_ethers()), false)
        .add_call(npm_contract.factory(), false);
    let (position, factory): (PositionsReturn, types::Address) = multicall.call().await?;
    let pool_contract = get_pool_contract(
        factory.to_alloy(),
        position.token_0.to_alloy(),
        position.token_1.to_alloy(),
        position.fee.into(),
        client.clone(),
    );
    multicall.clear_calls();
    multicall
        .add_call(pool_contract.slot_0(), false)
        .add_call(pool_contract.fee_growth_global_0x128(), false)
        .add_call(pool_contract.fee_growth_global_1x128(), false)
        .add_call(pool_contract.ticks(position.tick_lower), false)
        .add_call(pool_contract.ticks(position.tick_upper), false);
    let (
        Slot0Return { tick, .. },
        fee_growth_global_0x128,
        fee_growth_global_1x128,
        TicksReturn {
            fee_growth_outside_0x128: fee_growth_outside_0x128_lower,
            fee_growth_outside_1x128: fee_growth_outside_1x128_lower,
            ..
        },
        TicksReturn {
            fee_growth_outside_0x128: fee_growth_outside_0x128_upper,
            fee_growth_outside_1x128: fee_growth_outside_1x128_upper,
            ..
        },
    ): (
        Slot0Return,
        types::U256,
        types::U256,
        TicksReturn,
        TicksReturn,
    ) = multicall.call().await?;

    // https://github.com/Uniswap/v4-core/blob/f630c8ca8c669509d958353200953762fd15761a/contracts/libraries/Pool.sol#L566
    let (fee_growth_inside_0x128, fee_growth_inside_1x128) = if tick < position.tick_lower {
        (
            fee_growth_outside_0x128_lower - fee_growth_outside_0x128_upper,
            fee_growth_outside_1x128_lower - fee_growth_outside_1x128_upper,
        )
    } else if tick >= position.tick_upper {
        (
            fee_growth_outside_0x128_upper - fee_growth_outside_0x128_lower,
            fee_growth_outside_1x128_upper - fee_growth_outside_1x128_lower,
        )
    } else {
        (
            fee_growth_global_0x128
                - fee_growth_outside_0x128_lower
                - fee_growth_outside_0x128_upper,
            fee_growth_global_1x128
                - fee_growth_outside_1x128_lower
                - fee_growth_outside_1x128_upper,
        )
    };
    let (tokens_owed_0, tokens_owed_1) = get_tokens_owed(
        position.fee_growth_inside_0_last_x128.to_alloy(),
        position.fee_growth_inside_1_last_x128.to_alloy(),
        position.liquidity,
        fee_growth_inside_0x128.to_alloy(),
        fee_growth_inside_1x128.to_alloy(),
    );
    Ok((
        u128_to_uint256(position.tokens_owed_0) + tokens_owed_0,
        u128_to_uint256(position.tokens_owed_1) + tokens_owed_1,
    ))
}

/// Get the token SVG URL of the specified position.
///
/// ## Arguments
///
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `token_id`: The token id
/// * `client`: The client
/// * `block_id`: Optional block number to query
pub async fn get_token_svg<M: Middleware>(
    nonfungible_position_manager: Address,
    token_id: U256,
    client: Arc<M>,
    block_id: Option<BlockId>,
) -> Result<String, ContractError<M>> {
    let uri =
        get_nonfungible_position_manager_contract(nonfungible_position_manager, client.clone())
            .token_uri(token_id.to_ethers())
            .call_raw()
            .block(block_id.unwrap_or(BlockId::Number(BlockNumber::Latest)))
            .await?;
    let json_uri = general_purpose::URL_SAFE
        .decode(uri.replace("data:application/json;base64,", ""))
        .map_err(|e| abi::Error::Other(e.to_string().into()))
        .map_err(ContractError::DecodingError)?;
    let image = serde_json::from_slice::<serde_json::Value>(&json_uri)
        .map_err(abi::Error::SerdeJson)
        .map_err(ContractError::DecodingError)?
        .get("image")
        .unwrap()
        .to_string();
    Ok(image[1..image.len() - 1].to_string())
}

/// Predict the position after rebalance assuming the pool price remains the same.
///
/// ## Arguments
///
/// * `position`: Position info before rebalance.
/// * `new_tick_lower`: The new lower tick.
/// * `new_tick_upper`: The new upper tick.
pub fn get_rebalanced_position<P: Clone>(
    position: &mut Position<P>,
    new_tick_lower: i32,
    new_tick_upper: i32,
) -> Result<Position<P>> {
    let price = position.pool.token0_price();
    // Calculate the position equity denominated in token1 before rebalance.
    let equity_in_token1_before = price
        .quote(position.amount0()?)?
        .add(&position.amount1()?)?;
    let equity_before = fraction_to_big_decimal(&equity_in_token1_before);
    let price = fraction_to_big_decimal(&price);
    let token0_ratio = token0_price_to_ratio(price.clone(), new_tick_lower, new_tick_upper)?;
    let amount1_after = (BigDecimal::from(1) - token0_ratio) * &equity_before;
    // token0's equity denominated in token1 divided by the price
    let amount0_after = (equity_before - &amount1_after) / price;
    Position::from_amounts(
        position.pool.clone(),
        new_tick_lower,
        new_tick_upper,
        big_int_to_u256(amount0_after.to_bigint().unwrap()),
        big_int_to_u256(amount1_after.to_bigint().unwrap()),
        false,
    )
}

/// Predict the position if the pool price becomes the specified price.
///
/// ## Arguments
///
/// * `position`: Current position
/// * `new_price`: The new pool price
pub fn get_position_at_price<T, P>(
    position: Position<P>,
    new_price: BigDecimal,
) -> Result<Position<P>>
where
    T: TickTrait,
    P: TickDataProvider<Tick = T>,
{
    let sqrt_price_x96 = price_to_sqrt_ratio_x96(&new_price);
    let pool_at_new_price = Pool::new_with_tick_data_provider(
        position.pool.token0,
        position.pool.token1,
        position.pool.fee,
        sqrt_price_x96,
        position.pool.liquidity,
        position.pool.tick_data_provider,
    )?;
    Ok(Position::new(
        pool_at_new_price,
        position.liquidity,
        position.tick_lower,
        position.tick_upper,
    ))
}

/// Predict the position after rebalance assuming the pool price becomes the specified price.
///
/// ## Arguments
///
/// * `position`: Position info before rebalance.
/// * `new_price`: The new pool price
/// * `new_tick_lower`: The new lower tick.
/// * `new_tick_upper`: The new upper tick.
pub fn get_rebalanced_position_at_price<T, P>(
    position: Position<P>,
    new_price: BigDecimal,
    new_tick_lower: i32,
    new_tick_upper: i32,
) -> Result<Position<P>>
where
    T: TickTrait,
    P: TickDataProvider<Tick = T>,
{
    get_rebalanced_position(
        &mut get_position_at_price(position, new_price)?,
        new_tick_lower,
        new_tick_upper,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, uint};
    use num_traits::Signed;

    const NPM: Address = address!("C36442b4a4522E871399CD717aBDD847Ab11FE88");

    #[tokio::test]
    async fn test_from_token_id() {
        let position = Position::from_token_id(
            1,
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        assert_eq!(position.liquidity, 34399999543676);
        assert_eq!(position.tick_lower, 253320);
        assert_eq!(position.tick_upper, 264600);
    }

    #[tokio::test]
    async fn test_get_all_positions_by_owner() {
        let client = Arc::new(MAINNET.provider());
        let block_id = BlockId::from(17188000);
        let owner = address!("4bD047CA72fa05F0B89ad08FE5Ba5ccdC07DFFBF");
        let positions = get_all_positions_by_owner(NPM, owner, client.clone(), Some(block_id))
            .await
            .unwrap();
        let npm_contract = get_nonfungible_position_manager_contract(NPM, client.clone());
        let balance = npm_contract
            .balance_of(owner.to_ethers())
            .call_raw()
            .block(block_id)
            .await
            .unwrap()
            .as_usize();
        assert_eq!(positions.len(), balance);
        let mut multicall = Multicall::new_with_chain_id(client, None, Some(1u64)).unwrap();
        multicall.block = Some(block_id);
        multicall.add_calls(
            false,
            (0..balance).map(|i| {
                npm_contract.token_of_owner_by_index(owner.to_ethers(), types::U256::from(i))
            }),
        );
        let token_ids: Vec<types::U256> = multicall.call_array().await.unwrap();
        token_ids.into_iter().enumerate().for_each(|(i, token_id)| {
            assert_eq!(token_id, positions[i].token_id);
        });
    }

    #[tokio::test]
    async fn test_get_collectable_token_amounts() {
        let (tokens_owed_0, tokens_owed_1) = get_collectable_token_amounts(
            1,
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        assert_eq!(tokens_owed_0, uint!(3498422_U256));
        assert_eq!(tokens_owed_1, uint!(516299277575296150_U256));
    }

    #[tokio::test]
    async fn test_get_token_svg() {
        let svg = get_token_svg(
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        assert_eq!(
            svg[..60].to_string(),
            "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjkwIiBoZWlnaHQ9Ij"
        );
    }

    #[tokio::test]
    async fn test_get_rebalanced_position() {
        let mut position = get_position(
            1,
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        // rebalance to an out of range position
        let new_tick_lower = position.tick_upper;
        let new_tick_upper = new_tick_lower + 10 * FeeAmount::MEDIUM.tick_spacing();
        let mut new_position =
            get_rebalanced_position(&mut position, new_tick_lower, new_tick_upper).unwrap();
        assert!(new_position.amount1().unwrap().quotient().is_zero());
        let mut reverted_position =
            get_rebalanced_position(&mut new_position, position.tick_lower, position.tick_upper)
                .unwrap();
        let amount0 = position.amount0().unwrap().quotient();
        assert!(amount0 - reverted_position.amount0().unwrap().quotient() < BigInt::from(10));
        let amount1 = position.amount1().unwrap().quotient();
        assert!(
            &amount1 - reverted_position.amount1().unwrap().quotient()
                < amount1 / BigInt::from(1000000)
        );
        assert!(position.liquidity - reverted_position.liquidity < position.liquidity / 1000000);
    }

    #[tokio::test]
    async fn test_get_position_at_price() {
        let position = get_position(
            1,
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        // corresponds to tick -870686
        let small_price = BigDecimal::from_str("1.5434597458370203830544e-38").unwrap();
        let position = Position::new(
            Pool::new(
                position.pool.token0,
                position.pool.token1,
                FeeAmount::MEDIUM,
                uint!(797207963837958202618833735859_U256),
                4923530363713842_u128,
            )
            .unwrap(),
            68488980_u128,
            -887220,
            52980,
        );
        let mut position1 = get_position_at_price(position.clone(), small_price).unwrap();
        assert!(position1.amount0().unwrap().quotient().is_positive());
        assert!(position1.amount1().unwrap().quotient().is_zero());
        let mut position2 = get_position_at_price(
            position.clone(),
            fraction_to_big_decimal(
                &tick_to_price(
                    position.pool.token0,
                    position.pool.token1,
                    position.tick_upper,
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(position2.amount0().unwrap().quotient().is_zero());
        assert!(position2.amount1().unwrap().quotient().is_positive());
        let mut rebalanced_position =
            get_rebalanced_position(&mut position1, 46080, 62160).unwrap();
        assert!(rebalanced_position
            .amount0()
            .unwrap()
            .quotient()
            .is_positive());
        assert!(rebalanced_position.amount1().unwrap().quotient().is_zero());
    }

    #[tokio::test]
    async fn test_get_rebalanced_position_at_price() {
        let mut position = get_position(
            1,
            NPM,
            uint!(4_U256),
            Arc::new(MAINNET.provider()),
            Some(BlockId::from(17188000)),
        )
        .await
        .unwrap();
        // rebalance to an out of range position
        let new_tick_lower = position.tick_upper;
        let new_tick_upper = new_tick_lower + 10 * FeeAmount::MEDIUM.tick_spacing();
        let mut position_rebalanced_at_current_price =
            get_rebalanced_position(&mut position, new_tick_lower, new_tick_upper).unwrap();
        let price_upper = tick_to_price(
            position.pool.token0.clone(),
            position.pool.token1.clone(),
            position.tick_upper,
        )
        .unwrap();
        let mut position_rebalanced_at_tick_upper = get_rebalanced_position_at_price(
            position.clone(),
            fraction_to_big_decimal(&price_upper),
            new_tick_lower,
            new_tick_upper,
        )
        .unwrap();
        assert!(position_rebalanced_at_tick_upper
            .amount1()
            .unwrap()
            .quotient()
            .is_zero());
        // if rebalancing at the upper tick, `token0` are bought back at a higher price, hence
        // `amount0` will be lower
        assert!((position_rebalanced_at_current_price.amount0().unwrap()
            - position_rebalanced_at_tick_upper.amount0().unwrap())
        .quotient()
        .is_positive());
    }
}
