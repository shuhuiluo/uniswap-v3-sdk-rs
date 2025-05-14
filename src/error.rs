#![allow(clippy::missing_inline_in_public_items)]

#[cfg(doc)]
use crate::prelude::*;

use alloy_primitives::{aliases::I24, U160};
use uniswap_sdk_core::error::Error as CoreError;

#[derive(Debug, thiserror::Error)]
#[cfg_attr(not(feature = "extensions"), derive(Clone, Copy, Hash, PartialEq, Eq))]
pub enum Error {
    /// Thrown when an error occurs in the core library.
    #[error("{0}")]
    Core(#[from] CoreError),

    /// Thrown when the token passed to [`Pool::price_of`] is not one of the pool's tokens.
    #[error("Invalid token")]
    InvalidToken,

    /// Thrown when the tick passed to [`get_sqrt_ratio_at_tick`] is not between [`MIN_TICK`] and
    /// [`MAX_TICK`].
    #[error("Invalid tick: {0}")]
    InvalidTick(I24),

    /// Thrown when the price passed to [`get_tick_at_sqrt_ratio`] does not correspond to a price
    /// between [`MIN_TICK`] and [`MAX_TICK`].
    #[error("Invalid square root price: {0}")]
    InvalidSqrtPrice(U160),

    #[error("Invalid price or liquidity")]
    InvalidPriceOrLiquidity,

    #[error("Invalid price")]
    InvalidPrice,

    #[error("Overflow in full math mulDiv")]
    MulDivOverflow,

    #[error("Overflow when adding liquidity delta")]
    AddDeltaOverflow,

    #[error("Overflow when casting to U160")]
    SafeCastToU160Overflow,

    #[error("Overflow in price calculation")]
    PriceOverflow,

    #[error("Insufficient liquidity")]
    InsufficientLiquidity,

    #[error("No tick data provider was given")]
    NoTickDataError,

    #[error("{0}")]
    TickListError(#[from] TickListError),

    #[cfg(feature = "extensions")]
    #[error("Invalid tick range")]
    InvalidRange,

    #[cfg(feature = "extensions")]
    #[error("{0}")]
    ContractError(#[from] alloy::contract::Error),

    #[cfg(feature = "extensions")]
    #[error("{0}")]
    LensError(#[from] uniswap_lens::error::Error),

    #[cfg(feature = "extensions")]
    #[error("{0}")]
    MulticallError(#[from] alloy::providers::MulticallError),

    #[cfg(feature = "extensions")]
    #[error("Invalid access list")]
    InvalidAccessList,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, thiserror::Error)]
pub enum TickListError {
    #[error("Below smallest tick")]
    BelowSmallest,
    #[error("At or above largest tick")]
    AtOrAboveLargest,
    #[error("Not contained in tick list")]
    NotContained,
}

#[cfg(feature = "extensions")]
impl From<alloy::transports::TransportError> for Error {
    fn from(e: alloy::transports::TransportError) -> Self {
        Self::ContractError(alloy::contract::Error::TransportError(e))
    }
}
