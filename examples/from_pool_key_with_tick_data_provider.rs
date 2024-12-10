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
};
use alloy_primitives::{address, ruint::aliases::U256, U160};
use alloy_sol_types::SolValue;
use uniswap_sdk_core::{prelude::*, token};
use uniswap_v3_sdk::prelude::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let rpc_url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    let provider = ProviderBuilder::new().on_http(rpc_url);
    let block_id = BlockId::from(17000000);
    let wbtc = token!(1, "2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8, "WBTC");
    let weth = token!(1, "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, "WETH");

    // Create a pool with a tick map data provider
    let pool = Pool::<EphemeralTickMapDataProvider<i32>>::from_pool_key_with_tick_data_provider(
        1,
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
    let (local_amount_out, _pool_after) = pool.get_output_amount(&amount_in, None).unwrap();
    println!("Local amount out: {}", local_amount_out.quotient());

    let route = Route::new(vec![pool.clone()], wbtc, weth);
    let params = quote_call_parameters(
        &route,
        &amount_in,
        TradeType::ExactInput,
        Some(QuoteOptions {
            sqrt_price_limit_x96: U160::ZERO,
            use_quoter_v2: false,
        }),
    );
    let quoter_addr = *QUOTER_ADDRESSES.get(&1).unwrap();
    let tx = TransactionRequest {
        to: Some(quoter_addr.into()),
        input: params.calldata.into(),
        ..Default::default()
    };
    // Get the output amount from the quoter
    let res = provider.call(&tx).block(block_id).await.unwrap();
    let amount_out = U256::abi_decode(res.as_ref(), true).unwrap();
    println!("Quoter amount out: {}", amount_out);

    // Assert that the amounts are equal
    assert_eq!(U256::from_big_int(local_amount_out.quotient()), amount_out);
}
