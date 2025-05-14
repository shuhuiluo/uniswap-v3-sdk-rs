use alloy::{
    eips::BlockId,
    network::{Network, TransactionBuilder},
    node_bindings::WEI_IN_ETHER,
    providers::{ext::AnvilApi, Provider, ProviderBuilder},
    signers::{
        k256::ecdsa::SigningKey,
        local::{LocalSigner, PrivateKeySigner},
        SignerSync,
    },
    transports::http::reqwest::Url,
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
    let provider = ProviderBuilder::new().connect_anvil_with_config(|anvil| {
        anvil
            .fork(rpc_url)
            .fork_block_number(block_id.as_u64().unwrap())
    });
    provider.anvil_auto_impersonate_account(true).await.unwrap();
    let account: LocalSigner<SigningKey> = PrivateKeySigner::random();
    provider
        .anvil_set_balance(account.address(), WEI_IN_ETHER)
        .await
        .unwrap();
    let sender = provider.get_accounts().await.unwrap()[0];

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

    {
        // Set the state of the account to allow the position to be minted
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        let mut overrides = get_erc20_state_overrides(
            position.pool.token0.address(),
            account.address(),
            npm,
            amount0,
            &provider,
        )
        .await
        .unwrap();
        overrides.extend(
            get_erc20_state_overrides(
                position.pool.token1.address(),
                account.address(),
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
    }

    let token_id = mint_liquidity(&mut position, account.address(), &provider).await;

    let minted_position = Position::from_token_id(1, npm, token_id, provider.clone(), None)
        .await
        .unwrap();

    assert_eq!(minted_position.liquidity, position.liquidity);
    assert_eq!(minted_position.tick_lower, position.tick_lower);
    assert_eq!(minted_position.tick_upper, position.tick_upper);

    burn_liquidity(token_id, &position, &account, sender, &provider).await;

    assert_eq!(
        IERC721Enumerable::new(npm, provider)
            .balanceOf(account.address())
            .call()
            .await
            .unwrap(),
        U256::ZERO
    );
}

/// Mint a position
async fn mint_liquidity<N, P>(position: &mut Position, from: Address, provider: &P) -> U256
where
    N: Network,
    P: Provider<N>,
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
    let tx = N::TransactionRequest::default()
        .with_from(from)
        .with_to(npm)
        .with_input(params.calldata);
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    IERC721Enumerable::new(npm, provider)
        .tokenOfOwnerByIndex(from, U256::ZERO)
        .call()
        .await
        .unwrap()
}

/// Burn a position with a permit
async fn burn_liquidity<N, P>(
    token_id: U256,
    position: &Position,
    owner: &LocalSigner<SigningKey>,
    sender: Address,
    provider: &P,
) where
    N: Network,
    P: Provider<N>,
{
    let npm = *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES.get(&1).unwrap();

    // Sign the permit
    let hash = get_permit_data(
        NFTPermitValues {
            spender: sender,
            tokenId: token_id,
            nonce: U256::ZERO,
            deadline: U256::MAX,
        },
        npm,
        1,
    )
    .eip712_signing_hash();
    let signature = owner.sign_hash_sync(&hash).unwrap();

    let options = RemoveLiquidityOptions {
        token_id,
        liquidity_percentage: Percent::new(1, 1),
        slippage_tolerance: Percent::default(),
        deadline: U256::MAX,
        burn_token: true,
        permit: Some(NFTPermitOptions {
            signature,
            deadline: U256::MAX,
            spender: sender,
        }),
        collect_options: CollectOptions {
            token_id,
            expected_currency_owed0: CurrencyAmount::from_raw_amount(
                position.pool.token0.clone(),
                0,
            )
            .unwrap(),
            expected_currency_owed1: CurrencyAmount::from_raw_amount(
                position.pool.token1.clone(),
                0,
            )
            .unwrap(),
            recipient: owner.address(),
        },
    };
    let params = remove_call_parameters(position, options).unwrap();
    let tx = N::TransactionRequest::default()
        .with_from(sender)
        .with_to(npm)
        .with_input(params.calldata);
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
}
