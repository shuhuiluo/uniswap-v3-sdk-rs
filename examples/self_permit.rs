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
    node_bindings::WEI_IN_ETHER,
    providers::{Provider, ext::AnvilApi},
    rpc::types::TransactionRequest,
    signers::{SignerSync, local::PrivateKeySigner},
    sol,
};
use alloy_primitives::{B256, Signature, U256, keccak256};
use alloy_sol_types::SolValue;
use uniswap_v3_sdk::prelude::*;

#[path = "common/mod.rs"]
mod common;
use common::{CHAIN_ID, NPM_ADDRESS, USDC, USDC_ADDRESS, setup_anvil_fork_provider};

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
    // Create an Anvil fork
    let provider = setup_anvil_fork_provider();
    provider.anvil_auto_impersonate_account(true).await.unwrap();

    let usdc = USDC.clone();
    let npm = *NPM_ADDRESS;

    let iusdc = USDC::new(USDC_ADDRESS, provider.clone());
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
    let permit_data =
        get_erc20_permit_data(permit, name.leak(), version.leak(), USDC_ADDRESS, CHAIN_ID);
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
    println!("USDC allowance: {allowance}");
    assert_eq!(allowance, amount);
}
