use alloy::{
    eips::BlockId,
    providers::{ext::AnvilApi, Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    transports::{http::reqwest::Url, Transport},
};
use alloy_primitives::{address, Address, U256};
use uniswap_lens::bindings::ierc721enumerable::IERC721Enumerable;
use uniswap_sdk_core::{prelude::*, token};
use uniswap_v3_sdk::prelude::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let rpc_url: Url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    let block_id = BlockId::from(17000000);
    let wbtc = token!(
        1,
        address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
        8,
        "WBTC"
    );
    let weth = WETH9::on_chain(1).unwrap();
    let npm = *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES.get(&1).unwrap();

    // Create an Anvil fork
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_anvil_with_config(|anvil| {
            anvil
                .fork(rpc_url)
                .fork_block_number(block_id.as_u64().unwrap())
        });
    let account = provider.get_accounts().await.unwrap()[0];

    let pool = Pool::from_pool_key(
        1,
        FACTORY_ADDRESS,
        wbtc.address(),
        weth.address(),
        FeeAmount::LOW,
        provider.clone(),
        None,
    )
    .await
    .unwrap();
    let mut position = Position::new(
        pool.clone(),
        pool.liquidity,
        nearest_usable_tick(pool.tick_current - pool.tick_spacing(), pool.tick_spacing()),
        nearest_usable_tick(pool.tick_current + pool.tick_spacing(), pool.tick_spacing()),
    );

    // Set the state of the account to allow the position to be minted
    let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
    let mut overrides = get_erc20_state_overrides(
        position.pool.token0.address(),
        account,
        npm,
        amount0,
        &provider,
    )
    .await
    .unwrap();
    overrides.extend(
        get_erc20_state_overrides(
            position.pool.token1.address(),
            account,
            npm,
            amount1,
            &provider,
        )
        .await
        .unwrap(),
    );
    for (token, account_override) in overrides {
        for (slot, value) in account_override.state_diff.unwrap() {
            provider
                .anvil_set_storage_at(token, U256::from_be_bytes(slot.0), value)
                .await
                .unwrap();
        }
    }

    let minted_position = mint_liquidity(&mut position, account, &provider).await;

    assert_eq!(minted_position.liquidity, position.liquidity);
    assert_eq!(minted_position.tick_lower, position.tick_lower);
    assert_eq!(minted_position.tick_upper, position.tick_upper);
}

async fn mint_liquidity<T, P>(position: &mut Position, from: Address, provider: &P) -> Position
where
    T: Transport + Clone,
    P: Provider<T>,
{
    let npm = *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES.get(&1).unwrap();

    let options = AddLiquidityOptions {
        slippage_tolerance: Percent::default(),
        deadline: U256::MAX,
        use_native: None,
        token0_permit: None,
        token1_permit: None,
        specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
            recipient: from,
            create_pool: false,
        }),
    };
    let params = add_call_parameters(position, options).unwrap();
    let tx = TransactionRequest::default()
        .from(from)
        .to(npm)
        .input(params.calldata.into());
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    let token_id = IERC721Enumerable::new(npm, provider)
        .tokenOfOwnerByIndex(from, U256::ZERO)
        .call()
        .await
        .unwrap()
        ._0;
    Position::from_token_id(1, npm, token_id, provider, None)
        .await
        .unwrap()
}
