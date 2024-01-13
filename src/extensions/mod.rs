//! Extensions to the core library.

mod ephemeral_tick_data_provider;
mod price_tick_conversions;

pub use ephemeral_tick_data_provider::EphemeralTickDataProvider;
pub use price_tick_conversions::*;

use crate::prelude::*;
use alloy_primitives::U256;
use bigdecimal::BigDecimal;

pub fn u256_to_big_decimal(x: U256) -> BigDecimal {
    BigDecimal::from(u256_to_big_int(x))
}
