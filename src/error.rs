#[cfg(doc)]
use crate::prelude::*;

#[cfg(feature = "extensions")]
use alloy::contract::Error as ContractError;
#[cfg(feature = "extensions")]
use uniswap_lens::error::Error as LensError;

use alloy_primitives::{aliases::I24, U160};
use derive_more::From;
use uniswap_sdk_core::error::Error as CoreError;

#[derive(Debug, From)]
#[cfg_attr(not(feature = "extensions"), derive(Clone, Copy, Hash, PartialEq, Eq))]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum Error {
    /// Thrown when an error occurs in the core library.
    #[cfg_attr(feature = "std", error("{0}"))]
    Core(#[cfg_attr(not(feature = "std"), from)] CoreError),

    /// Thrown when the token passed to [`Pool::price_of`] is not one of the pool's tokens.
    #[cfg_attr(feature = "std", error("Invalid token"))]
    InvalidToken,

    /// Thrown when the tick passed to [`get_sqrt_ratio_at_tick`] is not between [`MIN_TICK`] and
    /// [`MAX_TICK`].
    #[cfg_attr(feature = "std", error("Invalid tick: {0}"))]
    InvalidTick(I24),

    /// Thrown when the price passed to [`get_tick_at_sqrt_ratio`] does not correspond to a price
    /// between [`MIN_TICK`] and [`MAX_TICK`].
    #[cfg_attr(feature = "std", error("Invalid square root price: {0}"))]
    InvalidSqrtPrice(U160),

    #[cfg_attr(feature = "std", error("Invalid price or liquidity"))]
    InvalidPriceOrLiquidity,

    #[cfg_attr(feature = "std", error("Invalid price"))]
    InvalidPrice,

    #[cfg(feature = "extensions")]
    #[cfg_attr(feature = "std", error("Invalid tick range"))]
    InvalidRange,

    #[cfg_attr(feature = "std", error("Overflow in full math mulDiv"))]
    MulDivOverflow,

    #[cfg_attr(feature = "std", error("Overflow when adding liquidity delta"))]
    AddDeltaOverflow,

    #[cfg_attr(feature = "std", error("Overflow when casting to U160"))]
    SafeCastToU160Overflow,

    #[cfg_attr(feature = "std", error("Overflow in price calculation"))]
    PriceOverflow,

    #[cfg_attr(feature = "std", error("Insufficient liquidity"))]
    InsufficientLiquidity,

    #[cfg_attr(feature = "std", error("No tick data provider was given"))]
    NoTickDataError,

    #[cfg(feature = "extensions")]
    #[cfg_attr(feature = "std", error("{0}"))]
    ContractError(#[cfg_attr(not(feature = "std"), from)] ContractError),

    #[cfg(feature = "extensions")]
    #[cfg_attr(feature = "std", error("{0}"))]
    LensError(#[cfg_attr(not(feature = "std"), from)] LensError),

    #[cfg_attr(feature = "std", error("{0}"))]
    TickListError(#[cfg_attr(not(feature = "std"), from)] TickListError),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum TickListError {
    #[cfg_attr(feature = "std", error("Below smallest tick"))]
    BelowSmallest,
    #[cfg_attr(feature = "std", error("At or above largest tick"))]
    AtOrAboveLargest,
    #[cfg_attr(feature = "std", error("Not contained in tick list"))]
    NotContained,
}
