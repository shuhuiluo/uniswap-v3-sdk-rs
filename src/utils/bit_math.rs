//! ## Bit Math Library in Rust
//!
//! This module is a Rust port of the Solidity [BitMath library](https://github.com/uniswap/v3-core/blob/main/contracts/libraries/BitMath.sol).

use alloy_primitives::Uint;

/// Trait to associate bit math functions with [`Uint`] types.
pub trait BitMath {
    #[must_use]
    fn most_significant_bit(self) -> usize;
    #[must_use]
    fn least_significant_bit(self) -> usize;
}

impl<const BITS: usize, const LIMBS: usize> BitMath for Uint<BITS, LIMBS> {
    #[inline]
    fn most_significant_bit(self) -> usize {
        most_significant_bit(self)
    }

    #[inline]
    fn least_significant_bit(self) -> usize {
        least_significant_bit(self)
    }
}

/// Returns the index of the most significant bit in a given [`Uint`].
///
/// ## Panics
///
/// Panics if the input is zero.
///
/// ## Arguments
///
/// * `x`: The [`Uint`] to find the most significant bit of.
///
/// ## Returns
///
/// The index of the most significant bit.
///
/// ## Examples
///
/// ```
/// use alloy_primitives::U160;
/// use uniswap_v3_sdk::prelude::most_significant_bit;
///
/// assert_eq!(
///     most_significant_bit(U160::from_str_radix("101010", 2).unwrap()),
///     5
/// );
/// assert_eq!(most_significant_bit(U160::MAX), 159);
/// ```
#[inline]
#[must_use]
pub fn most_significant_bit<const BITS: usize, const LIMBS: usize>(x: Uint<BITS, LIMBS>) -> usize {
    BITS - 1 - x.leading_zeros()
}

/// Returns the index of the least significant bit in a given [`Uint`].
///
/// ## Arguments
///
/// * `x`: The [`Uint`] to find the least significant bit of.
///
/// ## Returns
///
/// The index of the least significant bit.
///
/// ## Examples
///
/// ```
/// use alloy_primitives::U256;
/// use uniswap_v3_sdk::prelude::least_significant_bit;
///
/// assert_eq!(
///     least_significant_bit(U256::from_str_radix("101010", 2).unwrap()),
///     1
/// );
/// assert_eq!(least_significant_bit(U256::from(1) << 42), 42);
/// ```
#[inline]
#[must_use]
pub fn least_significant_bit<const BITS: usize, const LIMBS: usize>(x: Uint<BITS, LIMBS>) -> usize {
    x.trailing_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{uint, U256};

    const ONE: U256 = uint!(1_U256);

    #[test]
    #[should_panic(expected = "overflow")]
    fn most_significant_bit_throws_for_zero() {
        let _ = most_significant_bit(U256::ZERO);
    }

    #[test]
    fn test_most_significant_bit() {
        for i in 0..=255 {
            let x = ONE << i;
            assert_eq!(most_significant_bit(x), i);
        }
        for i in 1..=255 {
            let x = (ONE << i) - ONE;
            assert_eq!(most_significant_bit(x), i - 1);
        }
        assert_eq!(most_significant_bit(U256::MAX), 255);
    }

    #[test]
    fn test_least_significant_bit() {
        for i in 0..=255 {
            let x = ONE << i;
            assert_eq!(least_significant_bit(x), i);
        }
        for i in 1..=255 {
            let x = (ONE << i) - ONE;
            assert_eq!(least_significant_bit(x), 0);
        }
        assert_eq!(least_significant_bit(U256::MAX), 0);
    }
}
