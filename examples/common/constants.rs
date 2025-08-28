//! Common constants used across examples

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    transports::http::reqwest::Url,
};
use alloy_primitives::Address;
use once_cell::sync::Lazy;
use uniswap_sdk_core::prelude::NONFUNGIBLE_POSITION_MANAGER_ADDRESSES;

pub const CHAIN_ID: u64 = 1;
pub const FORK_BLOCK_NUMBER: u64 = 17000000;

pub const BLOCK_ID: BlockId = BlockId::Number(BlockNumberOrTag::Number(FORK_BLOCK_NUMBER));

pub static RPC_URL: Lazy<Url> = Lazy::new(|| {
    dotenv::dotenv().ok();
    std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap()
});

pub static NPM_ADDRESS: Lazy<Address> = Lazy::new(|| {
    *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES
        .get(&CHAIN_ID)
        .unwrap()
});
