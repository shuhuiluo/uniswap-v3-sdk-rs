use crate::error::Error;

/// Add a signed liquidity delta to liquidity and revert if it overflows or underflows
///
/// ## Arguments
///
/// * `x`: The liquidity before change
/// * `y`: The delta by which liquidity should be changed
///
/// ## Returns
///
/// The liquidity delta
#[inline]
pub fn add_delta(x: u128, y: i128) -> Result<u128, Error> {
    x.checked_add_signed(y).ok_or(Error::AddDeltaOverflow)
}
