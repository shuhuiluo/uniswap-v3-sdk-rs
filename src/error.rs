use alloy_primitives::{aliases::I24, U160};
use uniswap_sdk_core::error::Error as CoreError;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum Error {
    #[cfg_attr(feature = "std", error("{0}"))]
    Core(CoreError),

    /// Thrown when the tick passed to [`get_sqrt_price_at_tick`] is not between [`MIN_TICK`] and
    /// [`MAX_TICK`].
    #[cfg_attr(feature = "std", error("Invalid tick: {0}"))]
    InvalidTick(I24),

    /// Thrown when the price passed to [`get_tick_at_sqrt_price`] does not correspond to a price
    /// between [`MIN_TICK`] and [`MAX_TICK`].
    #[cfg_attr(feature = "std", error("Invalid square root price: {0}"))]
    InvalidSqrtPrice(U160),

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

    #[cfg_attr(feature = "std", error("Invalid price or liquidity"))]
    InvalidPriceOrLiquidity,

    #[cfg_attr(feature = "std", error("Invalid price"))]
    InvalidPrice,

    #[cfg_attr(feature = "std", error("No tick data provider was given"))]
    NoTickDataError,
}

impl From<CoreError> for Error {
    fn from(error: CoreError) -> Self {
        Error::Core(error)
    }
}
