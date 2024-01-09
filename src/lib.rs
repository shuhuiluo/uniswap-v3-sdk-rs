//! # uniswap-v3-sdk
//!
//! A Rust SDK for building applications on top of Uniswap V3.
//! Migration from the TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).

pub mod constants;
pub mod entities;
pub mod utils;

#[cfg(feature = "extensions")]
pub mod extensions;

pub mod prelude {
    pub use crate::{constants::*, entities::*, utils::*};

    #[cfg(feature = "extensions")]
    pub use crate::extensions::*;
}
