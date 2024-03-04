//! # uniswap-v3-sdk
//!
//! A Rust SDK for building applications on top of Uniswap V3.
//! Migration from the TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).
//!
//! ## Features
//!
//! - Opinionated Rust implementation of the Uniswap V3 SDK with a focus on readability and
//!   performance
//! - Usage of [alloy-rs](https://github.com/alloy-rs) types
//! - Reimplementation of the math libraries in [Uniswap V3 Math In Rust](https://github.com/0xKitsune/uniswap-v3-math)
//!   based on optimizations presented in [Uni V3 Lib](https://github.com/Aperture-Finance/uni-v3-lib)
//! - Extensive unit tests and benchmarks
//! - An [`extensions`](./src/extensions) feature for additional functionalities related to Uniswap
//!   V3, including:
//!
//!     - [`pool`](./src/extensions/pool.rs) module for creating a `Pool` struct from a pool key and
//!       fetching the liquidity map within a tick range for the specified pool, using RPC client
//!     - [`position`](./src/extensions/position.rs) module for creating a `Position` struct from a
//!       token id and fetching the state and pool for all positions of the specified owner, using
//!       RPC client, etc
//!     - [`price_tick_conversions`](./src/extensions/price_tick_conversions.rs) module for
//!       converting between prices and ticks
//!     - [`ephemeral_tick_data_provider`](./src/extensions/ephemeral_tick_data_provider.rs) module for fetching ticks using
//!       an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
//!       in a single `eth_call`

pub mod abi;
pub mod constants;
pub mod entities;
pub mod multicall;
pub mod nonfungible_position_manager;
pub mod payments;
pub mod quoter;
pub mod self_permit;
pub mod staker;
pub mod swap_router;
pub mod utils;

#[cfg(feature = "extensions")]
pub mod extensions;

#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::{
        abi::*, constants::*, entities::*, multicall::encode_multicall,
        nonfungible_position_manager::*, payments::*, quoter::*, self_permit::*, staker::*,
        swap_router::*, utils::*,
    };

    #[cfg(feature = "extensions")]
    pub use crate::extensions::*;
}
