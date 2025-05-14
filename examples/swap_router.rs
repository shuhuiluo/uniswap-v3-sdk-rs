//! Example demonstrating how to swap tokens with the swap router
//!
//! # Prerequisites
//! - Environment variable MAINNET_RPC_URL must be set
//! - Requires the "extensions" feature
//!
//! # Note
//! This example uses mainnet block 17000000 for consistent results

use alloy::{
    eips::BlockId,
    node_bindings::WEI_IN_ETHER,
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    sol,
    transports::http::reqwest::Url,
};
use alloy_primitives::address;
use alloy_sol_types::SolCall;
use uniswap_sdk_core::{prelude::*, token};
use uniswap_v3_sdk::prelude::*;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address target) returns (uint256);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let rpc_url: Url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(rpc_url.clone());
    let block_id = BlockId::from(17000000);
    const WBTC: Address = address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
    let wbtc = token!(1, WBTC, 8, "WBTC");
    let eth = Ether::on_chain(1);

    // Create a pool with a tick map data provider
    let pool = Pool::<EphemeralTickMapDataProvider>::from_pool_key_with_tick_data_provider(
        1,
        FACTORY_ADDRESS,
        wbtc.address(),
        eth.address(),
        FeeAmount::LOW,
        provider.clone(),
        Some(block_id),
    )
    .await
    .unwrap();
    let amount_in =
        CurrencyAmount::from_raw_amount(eth.clone(), WEI_IN_ETHER.to_big_int()).unwrap();

    // Get the output amount from the quoter
    let route = Route::new(vec![pool], eth, wbtc);
    let params = quote_call_parameters(&route, &amount_in, TradeType::ExactInput, None);
    let tx = TransactionRequest::default()
        .to(*QUOTER_ADDRESSES.get(&1).unwrap())
        .input(params.calldata.into());
    let res = provider.call(tx).block(block_id).await.unwrap();
    let amount_out =
        IQuoter::quoteExactInputSingleCall::abi_decode_returns_validate(res.as_ref()).unwrap();
    println!("Quoter amount out: {}", amount_out);

    // Create an Anvil fork
    let provider = ProviderBuilder::new().connect_anvil_with_config(|anvil| {
        anvil
            .fork(rpc_url)
            .fork_block_number(block_id.as_u64().unwrap())
    });
    let account = provider.get_accounts().await.unwrap()[0];

    // Build the swap transaction
    let trade = Trade::from_route(route, amount_in, TradeType::ExactInput)
        .await
        .unwrap();
    let params = swap_call_parameters(
        &mut [trade],
        SwapOptions {
            recipient: account,
            ..Default::default()
        },
    )
    .unwrap();
    let tx = TransactionRequest::default()
        .from(account)
        .to(*SWAP_ROUTER_02_ADDRESSES.get(&1).unwrap())
        .input(params.calldata.into())
        .value(params.value);

    // Execute the swap
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    let iwbtc = IERC20::new(WBTC, provider);
    let balance = iwbtc.balanceOf(account).call().await.unwrap();
    println!("WBTC balance: {}", balance);
    assert_eq!(balance, amount_out);
}
