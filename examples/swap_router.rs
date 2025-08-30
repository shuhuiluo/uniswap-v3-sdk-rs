//! Example demonstrating how to swap tokens with the swap router
//!
//! # Prerequisites
//! - Environment variable MAINNET_RPC_URL must be set
//! - Requires the "extensions" feature
//!
//! # Note
//! This example uses mainnet block 17000000 for consistent results

use alloy::{
    node_bindings::WEI_IN_ETHER, providers::Provider, rpc::types::TransactionRequest, sol,
};
use alloy_sol_types::SolCall;
use uniswap_sdk_core::prelude::*;
use uniswap_v3_sdk::prelude::*;

#[path = "common/mod.rs"]
mod common;
use common::{
    setup_anvil_fork_provider, setup_http_provider, BLOCK_ID, CHAIN_ID, ETHER, WBTC, WBTC_ADDRESS,
};

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address target) returns (uint256);
    }
}

#[tokio::main]
async fn main() {
    let provider = setup_http_provider();
    let wbtc = WBTC.clone();
    let eth = ETHER.clone();

    // Create a pool with a tick map data provider
    let pool = Pool::<EphemeralTickMapDataProvider>::from_pool_key_with_tick_data_provider(
        CHAIN_ID,
        FACTORY_ADDRESS,
        wbtc.address(),
        eth.address(),
        FeeAmount::LOW,
        provider.clone(),
        Some(BLOCK_ID),
    )
    .await
    .unwrap();
    let amount_in =
        CurrencyAmount::from_raw_amount(eth.clone(), WEI_IN_ETHER.to_big_int()).unwrap();

    // Get the output amount from the quoter
    let route = Route::new(vec![pool], eth, wbtc);
    let params = quote_call_parameters(&route, &amount_in, TradeType::ExactInput, None);
    let tx = TransactionRequest::default()
        .to(*QUOTER_ADDRESSES.get(&CHAIN_ID).unwrap())
        .input(params.calldata.into());
    let res = provider.call(tx).block(BLOCK_ID).await.unwrap();
    let amount_out =
        IQuoter::quoteExactInputSingleCall::abi_decode_returns_validate(res.as_ref()).unwrap();
    println!("Quoter amount out: {amount_out}");

    // Create an Anvil fork
    let provider = setup_anvil_fork_provider();
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
        .to(*SWAP_ROUTER_02_ADDRESSES.get(&CHAIN_ID).unwrap())
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

    let iwbtc = IERC20::new(WBTC_ADDRESS, provider);
    let balance = iwbtc.balanceOf(account).call().await.unwrap();
    println!("WBTC balance: {balance}");
    assert_eq!(balance, amount_out);
}
