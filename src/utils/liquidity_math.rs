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
pub fn add_delta(x: u128, y: i128) -> Result<u128, Error> {
    if y < 0 {
        let z = x.overflowing_sub(-y as u128);

        if z.1 {
            Err(Error::AddDeltaOverflow)
        } else {
            Ok(z.0)
        }
    } else {
        let z = x.overflowing_add(y as u128);
        if z.0 < x {
            Err(Error::AddDeltaOverflow)
        } else {
            Ok(z.0)
        }
    }
}
