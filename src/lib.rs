//! # uniswap-v3-sdk
//!
//! A Rust SDK for building applications on top of Uniswap V3.
//! Migration from the TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).

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

pub mod prelude {
    pub use crate::{
        abi::*, constants::*, entities::*, multicall::encode_multicall,
        nonfungible_position_manager::*, payments::*, quoter::*, self_permit::*, staker::*,
        swap_router::*, utils::*,
    };

    #[cfg(feature = "extensions")]
    pub use crate::extensions::*;
}
