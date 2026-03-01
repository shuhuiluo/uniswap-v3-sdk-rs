//! Common token definitions used across examples

use super::constants::CHAIN_ID;
use alloy_primitives::{Address, address};
use once_cell::sync::Lazy;
use uniswap_sdk_core::{prelude::*, token};

// Token addresses
pub const WBTC_ADDRESS: Address = address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
pub const USDC_ADDRESS: Address = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

pub static WBTC: Lazy<Token> = Lazy::new(|| token!(CHAIN_ID, WBTC_ADDRESS, 8, "WBTC"));

pub static WETH: Lazy<Token> = Lazy::new(|| WETH9::on_chain(CHAIN_ID).unwrap());

pub static USDC: Lazy<Token> = Lazy::new(|| token!(CHAIN_ID, USDC_ADDRESS, 6, "USDC"));

pub static ETHER: Lazy<Ether> = Lazy::new(|| Ether::on_chain(CHAIN_ID));
