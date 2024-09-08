//! ## Position Extension
//! This module provides functions to create a [`Position`] struct from the token id, get the state
//! and pool for all positions of the specified owner by deploying an ephemeral contract via
//! `eth_call`, etc.

use crate::prelude::{Error, *};
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    providers::Provider,
    transports::Transport,
};
use alloy_primitives::{Address, ChainId, U256};
use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use uniswap_lens::{
    position_lens,
    prelude::{
        ephemeralallpositionsbyowner::EphemeralAllPositionsByOwner,
        ephemeralgetposition::EphemeralGetPosition,
        iuniswapv3nonfungiblepositionmanager::IUniswapV3NonfungiblePositionManager::{
            positionsReturn, IUniswapV3NonfungiblePositionManagerInstance,
        },
    },
};
use uniswap_sdk_core::{prelude::*, token};

pub const fn get_nonfungible_position_manager_contract<T, P>(
    nonfungible_position_manager: Address,
    provider: P,
) -> IUniswapV3NonfungiblePositionManagerInstance<T, P>
where
    T: Transport + Clone,
    P: Provider<T>,
{
    IUniswapV3NonfungiblePositionManagerInstance::new(nonfungible_position_manager, provider)
}

/// Get a [`Position`] struct from the token id
///
/// ## Arguments
///
/// * `chain_id`: The chain id
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `token_id`: The token id
/// * `provider`: The alloy provider
/// * `block_id`: Optional block number to query
pub async fn get_position<T, P>(
    chain_id: ChainId,
    nonfungible_position_manager: Address,
    token_id: U256,
    provider: P,
    block_id: Option<BlockId>,
) -> Result<Position, Error>
where
    T: Transport + Clone,
    P: Provider<T> + Clone,
{
    let block_id_ = block_id.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
    let npm_contract =
        get_nonfungible_position_manager_contract(nonfungible_position_manager, provider.clone());
    // TODO: use multicall
    let factory = npm_contract.factory().block(block_id_).call().await?._0;
    let position = npm_contract
        .positions(token_id)
        .block(block_id_)
        .call()
        .await?;
    let positionsReturn {
        token0,
        token1,
        fee,
        tickLower: tick_lower,
        tickUpper: tick_upper,
        liquidity,
        ..
    } = position;
    let pool = get_pool(
        chain_id,
        factory,
        token0,
        token1,
        fee.into(),
        provider,
        block_id,
    )
    .await?;
    Ok(Position::new(
        pool,
        liquidity,
        tick_lower.as_i32(),
        tick_upper.as_i32(),
    ))
}

impl Position {
    /// Get a [`Position`] struct from the token id in a single call by deploying an ephemeral
    /// contract via `eth_call`
    ///
    /// ## Arguments
    ///
    /// * `chain_id`: The chain id
    /// * `nonfungible_position_manager`: The nonfungible position manager address
    /// * `token_id`: The token id
    /// * `provider`: The alloy provider
    /// * `block_id`: Optional block number to query
    pub async fn from_token_id<T, P>(
        chain_id: ChainId,
        nonfungible_position_manager: Address,
        token_id: U256,
        provider: P,
        block_id: Option<BlockId>,
    ) -> Result<Self, Error>
    where
        T: Transport + Clone,
        P: Provider<T>,
    {
        let EphemeralGetPosition::PositionState {
            position,
            slot0,
            activeLiquidity: active_liquidity,
            decimals0,
            decimals1,
            ..
        } = position_lens::get_position_details(
            nonfungible_position_manager,
            token_id,
            provider,
            block_id,
        )
        .await
        .map_err(|_| Error::LensError)?;
        let pool = Pool::new(
            token!(chain_id, position.token0, decimals0),
            token!(chain_id, position.token1, decimals1),
            position.fee.into(),
            slot0.sqrtPriceX96,
            active_liquidity,
        )?;
        Ok(Position::new(
            pool,
            position.liquidity,
            position.tickLower.as_i32(),
            position.tickUpper.as_i32(),
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
/// * `provider`: The alloy provider
/// * `block_id`: Optional block number to query
pub async fn get_all_positions_by_owner<T, P>(
    nonfungible_position_manager: Address,
    owner: Address,
    provider: P,
    block_id: Option<BlockId>,
) -> Result<Vec<EphemeralAllPositionsByOwner::PositionState>>
where
    T: Transport + Clone,
    P: Provider<T>,
{
    position_lens::get_all_positions_by_owner(
        nonfungible_position_manager,
        owner,
        provider,
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
/// * `provider`: The alloy provider
/// * `block_id`: Optional block number to query
///
/// ## Returns
///
/// A tuple of the collectable token amounts.
pub async fn get_collectable_token_amounts<T, P>(
    _chain_id: ChainId,
    nonfungible_position_manager: Address,
    token_id: U256,
    provider: P,
    block_id: Option<BlockId>,
) -> Result<(U256, U256)>
where
    T: Transport + Clone,
    P: Provider<T> + Clone,
{
    let block_id_ = block_id.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));
    let npm_contract =
        get_nonfungible_position_manager_contract(nonfungible_position_manager, provider.clone());
    // TODO: use multicall
    let factory = npm_contract.factory().block(block_id_).call().await?._0;
    let position = npm_contract
        .positions(token_id)
        .block(block_id_)
        .call()
        .await?;
    let pool_contract = get_pool_contract(
        factory,
        position.token0,
        position.token1,
        position.fee.into(),
        provider,
    );
    let tick = pool_contract.slot0().block(block_id_).call().await?.tick;
    let fee_growth_global_0x128 = pool_contract
        .feeGrowthGlobal0X128()
        .block(block_id_)
        .call()
        .await?
        ._0;
    let fee_growth_global_1x128 = pool_contract
        .feeGrowthGlobal1X128()
        .block(block_id_)
        .call()
        .await?
        ._0;
    let tick_info_lower = pool_contract
        .ticks(position.tickLower)
        .block(block_id_)
        .call()
        .await?;
    let fee_growth_outside_0x128_lower = tick_info_lower.feeGrowthOutside0X128;
    let fee_growth_outside_1x128_lower = tick_info_lower.feeGrowthOutside1X128;
    let tick_info_upper = pool_contract
        .ticks(position.tickUpper)
        .block(block_id_)
        .call()
        .await?;
    let fee_growth_outside_0x128_upper = tick_info_upper.feeGrowthOutside0X128;
    let fee_growth_outside_1x128_upper = tick_info_upper.feeGrowthOutside1X128;

    // https://github.com/Uniswap/v4-core/blob/f630c8ca8c669509d958353200953762fd15761a/contracts/libraries/Pool.sol#L566
    let (fee_growth_inside_0x128, fee_growth_inside_1x128) = if tick < position.tickLower {
        (
            fee_growth_outside_0x128_lower - fee_growth_outside_0x128_upper,
            fee_growth_outside_1x128_lower - fee_growth_outside_1x128_upper,
        )
    } else if tick >= position.tickUpper {
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
        position.feeGrowthInside0LastX128,
        position.feeGrowthInside1LastX128,
        position.liquidity,
        fee_growth_inside_0x128,
        fee_growth_inside_1x128,
    );
    Ok((
        U256::from(position.tokensOwed0) + tokens_owed_0,
        U256::from(position.tokensOwed1) + tokens_owed_1,
    ))
}

/// Get the token SVG URL of the specified position.
///
/// ## Arguments
///
/// * `nonfungible_position_manager`: The nonfungible position manager address
/// * `token_id`: The token id
/// * `provider`: The alloy provider
/// * `block_id`: Optional block number to query
pub async fn get_token_svg<T, P>(
    nonfungible_position_manager: Address,
    token_id: U256,
    provider: P,
    block_id: Option<BlockId>,
) -> Result<String>
where
    T: Transport + Clone,
    P: Provider<T>,
{
    let uri = get_nonfungible_position_manager_contract(nonfungible_position_manager, provider)
        .tokenURI(token_id)
        .block(block_id.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest)))
        .call()
        .await?
        ._0;
    let json_uri =
        general_purpose::URL_SAFE.decode(uri.replace("data:application/json;base64,", ""))?;
    let image = serde_json::from_slice::<serde_json::Value>(&json_uri)?
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
pub fn get_rebalanced_position<TP: TickDataProvider>(
    position: &mut Position<TP>,
    new_tick_lower: TP::Index,
    new_tick_upper: TP::Index,
) -> Result<Position<TP>, Error> {
    let price = position.pool.token0_price();
    // Calculate the position equity denominated in token1 before rebalance.
    let equity_in_token1_before = price
        .quote(&position.amount0_cached()?)?
        .add(&position.amount1_cached()?)?;
    let equity_before = fraction_to_big_decimal(&equity_in_token1_before);
    let price = fraction_to_big_decimal(&price);
    let token0_ratio = token0_price_to_ratio(
        price.clone(),
        new_tick_lower.to_i24(),
        new_tick_upper.to_i24(),
    )?;
    let amount1_after = (BigDecimal::from(1) - token0_ratio) * &equity_before;
    // token0's equity denominated in token1 divided by the price
    let amount0_after = (equity_before - &amount1_after) / price;
    Position::from_amounts(
        position.pool.clone(),
        new_tick_lower,
        new_tick_upper,
        U256::from_big_int(amount0_after.to_bigint().unwrap()),
        U256::from_big_int(amount1_after.to_bigint().unwrap()),
        false,
    )
}

/// Predict the position if the pool price becomes the specified price.
///
/// ## Arguments
///
/// * `position`: Current position
/// * `new_price`: The new pool price
pub fn get_position_at_price<TP>(
    position: Position<TP>,
    new_price: BigDecimal,
) -> Result<Position<TP>, Error>
where
    TP: TickDataProvider,
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
pub fn get_rebalanced_position_at_price<TP>(
    position: Position<TP>,
    new_price: BigDecimal,
    new_tick_lower: TP::Index,
    new_tick_upper: TP::Index,
) -> Result<Position<TP>, Error>
where
    TP: TickDataProvider,
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
    use crate::tests::PROVIDER;
    use alloy_primitives::{address, uint};
    use num_traits::Signed;

    const NPM: Address = address!("C36442b4a4522E871399CD717aBDD847Ab11FE88");
    const BLOCK_ID: Option<BlockId> = Some(BlockId::Number(BlockNumberOrTag::Number(17188000)));

    #[tokio::test]
    async fn test_from_token_id() {
        let position = Position::from_token_id(1, NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
            .await
            .unwrap();
        assert_eq!(position.liquidity, 34399999543676);
        assert_eq!(position.tick_lower, 253320);
        assert_eq!(position.tick_upper, 264600);
    }

    #[tokio::test]
    async fn test_get_all_positions_by_owner() {
        let provider = PROVIDER.clone();
        let block_id = BlockId::from(17188000);
        let owner = address!("4bD047CA72fa05F0B89ad08FE5Ba5ccdC07DFFBF");
        let positions = get_all_positions_by_owner(NPM, owner, provider.clone(), Some(block_id))
            .await
            .unwrap();
        let npm_contract = get_nonfungible_position_manager_contract(NPM, provider.clone());
        let balance = npm_contract
            .balanceOf(owner)
            .block(block_id)
            .call()
            .await
            .unwrap()
            .balance
            .into_limbs()[0] as usize;
        assert_eq!(positions.len(), balance);
        // let mut multicall = Multicall::new_with_chain_id(provider, None, Some(1u64)).unwrap();
        // multicall.block = Some(block_id);
        // multicall.add_calls(
        //     false,
        //     (0..balance).map(|i| npm_contract.token_of_owner_by_index(owner,
        // types::U256::from(i))), );
        // let token_ids: Vec<types::U256> = multicall.call_array().await.unwrap();
        // token_ids.into_iter().enumerate().for_each(|(i, token_id)| {
        //     assert_eq!(token_id, positions[i].token_id);
        // });
    }

    #[tokio::test]
    async fn test_get_collectable_token_amounts() {
        let (tokens_owed_0, tokens_owed_1) =
            get_collectable_token_amounts(1, NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
                .await
                .unwrap();
        assert_eq!(tokens_owed_0, uint!(3498422_U256));
        assert_eq!(tokens_owed_1, uint!(516299277575296150_U256));
    }

    #[tokio::test]
    async fn test_get_token_svg() {
        let svg = get_token_svg(NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
            .await
            .unwrap();
        assert_eq!(
            svg[..60].to_string(),
            "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjkwIiBoZWlnaHQ9Ij"
        );
    }

    #[tokio::test]
    async fn test_get_rebalanced_position() {
        let mut position = get_position(1, NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
            .await
            .unwrap();
        // rebalance to an out of range position
        let new_tick_lower = position.tick_upper;
        let new_tick_upper = new_tick_lower + 10 * FeeAmount::MEDIUM.tick_spacing().as_i32();
        let mut new_position =
            get_rebalanced_position(&mut position, new_tick_lower, new_tick_upper).unwrap();
        assert!(new_position.amount1().unwrap().quotient().is_zero());
        let reverted_position =
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
        let position = get_position(1, NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
            .await
            .unwrap();
        // corresponds to tick -870686
        let small_price = BigDecimal::from_str("1.5434597458370203830544e-38").unwrap();
        let position = Position::new(
            Pool::new(
                position.pool.token0,
                position.pool.token1,
                FeeAmount::MEDIUM,
                uint!(797207963837958202618833735859_U160),
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
        let position2 = get_position_at_price(
            position.clone(),
            fraction_to_big_decimal(
                &tick_to_price(
                    position.pool.token0,
                    position.pool.token1,
                    position.tick_upper.try_into().unwrap(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(position2.amount0().unwrap().quotient().is_zero());
        assert!(position2.amount1().unwrap().quotient().is_positive());
        let rebalanced_position = get_rebalanced_position(&mut position1, 46080, 62160).unwrap();
        assert!(rebalanced_position
            .amount0()
            .unwrap()
            .quotient()
            .is_positive());
        assert!(rebalanced_position.amount1().unwrap().quotient().is_zero());
    }

    #[tokio::test]
    async fn test_get_rebalanced_position_at_price() {
        let mut position = get_position(1, NPM, uint!(4_U256), PROVIDER.clone(), BLOCK_ID)
            .await
            .unwrap();
        // rebalance to an out of range position
        let new_tick_lower = position.tick_upper;
        let new_tick_upper = new_tick_lower + 10 * FeeAmount::MEDIUM.tick_spacing().as_i32();
        let position_rebalanced_at_current_price =
            get_rebalanced_position(&mut position, new_tick_lower, new_tick_upper).unwrap();
        let price_upper = tick_to_price(
            position.pool.token0.clone(),
            position.pool.token1.clone(),
            position.tick_upper.try_into().unwrap(),
        )
        .unwrap();
        let position_rebalanced_at_tick_upper = get_rebalanced_position_at_price(
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
