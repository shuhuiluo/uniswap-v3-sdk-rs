//! Example demonstrating how to sign an ERC20 permit and encode the `selfPermit` call to the
//! Uniswap V3 NonfungiblePositionManager
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
    providers::{ext::AnvilApi, Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::{local::PrivateKeySigner, SignerSync},
    sol,
    transports::http::reqwest::Url,
};
use alloy_primitives::{keccak256, Signature, B256, U256};
use alloy_sol_types::SolValue;
use uniswap_sdk_core::{prelude::*, token};
use uniswap_v3_sdk::prelude::*;

sol! {
    #[sol(rpc)]
    interface USDC {
        function name() returns (string);
        function version() returns (string);
        function allowance(address owner, address spender) returns (uint256);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let rpc_url: Url = std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap();
    let block_id = BlockId::from(17000000);

    // Create an Anvil fork
    let provider = ProviderBuilder::new().connect_anvil_with_config(|anvil| {
        anvil
            .fork(rpc_url)
            .fork_block_number(block_id.as_u64().unwrap())
    });
    provider.anvil_auto_impersonate_account(true).await.unwrap();

    let usdc = token!(1, "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 6);
    let npm = *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES.get(&1).unwrap();

    let iusdc = USDC::new(usdc.address(), provider.clone());
    let name = iusdc.name().call().await.unwrap();
    let version = iusdc.version().call().await.unwrap();

    // Create a signer and sign a permit
    let signer = PrivateKeySigner::random();
    let amount = U256::from_be_slice(keccak256(signer.address().abi_encode()).as_slice());
    let permit = IERC20Permit::Permit {
        owner: signer.address(),
        spender: npm,
        value: amount,
        nonce: U256::ZERO,
        deadline: U256::MAX,
    };
    let permit_data = get_erc20_permit_data(permit, name.leak(), version.leak(), usdc.address(), 1);
    let hash: B256 = permit_data.eip712_signing_hash();
    let signature: Signature = signer.sign_hash_sync(&hash).unwrap();
    assert_eq!(
        signature.recover_address_from_prehash(&hash).unwrap(),
        signer.address()
    );

    // Encode the permit calldata
    let options = PermitOptions::Standard(StandardPermitArguments {
        signature,
        amount,
        deadline: U256::MAX,
    });
    let calldata = encode_permit(&usdc, options);

    // Set the signer balance and send the transaction
    provider
        .anvil_set_balance(signer.address(), WEI_IN_ETHER)
        .await
        .unwrap();
    let tx = TransactionRequest::default()
        .from(signer.address())
        .to(npm)
        .input(calldata.into());
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Check the spender allowance
    let allowance = iusdc.allowance(signer.address(), npm).call().await.unwrap();
    println!("USDC allowance: {}", allowance);
    assert_eq!(allowance, amount);
}
