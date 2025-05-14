//! Example demonstrating pool creation with tick data provider and swap simulation
//!
//! # Prerequisites
//! - Environment variable MAINNET_RPC_URL must be set
//! - Requires the "extensions" feature
//!
//! # Note
//! This example uses mainnet block 17000000 for consistent results

use alloy::{
    eips::BlockId,
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    transports::http::reqwest::Url,
};
use alloy_primitives::U256;
use alloy_sol_types::SolCall;
use uniswap_sdk_core::{prelude::*, token};
use uniswap_v3_sdk::prelude::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let rpc_url: Url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().connect_http(rpc_url);
    let block_id = BlockId::from(17000000);
    const CHAIN_ID: u64 = 1;
    let wbtc = token!(
        CHAIN_ID,
        "2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
        8,
        "WBTC"
    );
    let weth = WETH9::on_chain(CHAIN_ID).unwrap();

    // Create a pool with a tick map data provider
    let pool = Pool::<EphemeralTickMapDataProvider>::from_pool_key_with_tick_data_provider(
        CHAIN_ID,
        FACTORY_ADDRESS,
        wbtc.address(),
        weth.address(),
        FeeAmount::LOW,
        provider.clone(),
        Some(block_id),
    )
    .await
    .unwrap();
    // Get the output amount from the pool
    let amount_in = CurrencyAmount::from_raw_amount(wbtc.clone(), 100000000).unwrap();
    let local_amount_out = pool.get_output_amount(&amount_in, None).await.unwrap();
    let local_amount_out = local_amount_out.quotient();
    println!("Local amount out: {}", local_amount_out);

    // Get the output amount from the quoter
    let route = Route::new(vec![pool], wbtc, weth);
    let params = quote_call_parameters(&route, &amount_in, TradeType::ExactInput, None);
    let tx = TransactionRequest::default()
        .to(*QUOTER_ADDRESSES.get(&CHAIN_ID).unwrap())
        .input(params.calldata.into());
    let res = provider.call(tx).block(block_id).await.unwrap();
    let amount_out =
        IQuoter::quoteExactInputSingleCall::abi_decode_returns_validate(res.as_ref()).unwrap();
    println!("Quoter amount out: {}", amount_out);

    // Compare local calculation with on-chain quoter to ensure accuracy
    assert_eq!(U256::from_big_int(local_amount_out), amount_out);
}
