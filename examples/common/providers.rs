//! Common provider setup utilities for examples

use super::constants::{FORK_BLOCK_NUMBER, RPC_URL};
use alloy::{
    providers::{Provider, ProviderBuilder},
    signers::{k256::ecdsa::SigningKey, local::LocalSigner},
};

pub fn setup_http_provider() -> impl Provider + Clone {
    ProviderBuilder::new().connect_http(RPC_URL.clone())
}

pub async fn setup_anvil_fork_provider() -> impl Provider + Clone {
    ProviderBuilder::new().connect_anvil_with_config(|anvil| {
        anvil
            .fork(RPC_URL.clone())
            .fork_block_number(FORK_BLOCK_NUMBER)
    })
}

pub fn random_signer() -> LocalSigner<SigningKey> {
    alloy::signers::local::PrivateKeySigner::random()
}
