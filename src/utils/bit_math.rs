//! ## Bit Math Library in Rust
//!
//! This module is a Rust port of the Solidity [BitMath library](https://github.com/uniswap/v3-core/blob/main/contracts/libraries/BitMath.sol).

use alloy_primitives::U256;

pub fn most_significant_bit(x: U256) -> u8 {
    if x.is_zero() {
        panic!("ZERO")
    }
    255 - x.leading_zeros() as u8
}

pub fn least_significant_bit(x: U256) -> u8 {
    if x.is_zero() {
        panic!("ZERO")
    }
    x.trailing_zeros() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::{Shl, Sub};

    #[test]
    #[should_panic(expected = "ZERO")]
    fn most_significant_bit_throws_for_zero() {
        let _ = most_significant_bit(U256::ZERO);
    }

    #[test]
    fn test_most_significant_bit() {
        for i in 1u8..=255 {
            let x = U256::from(1).shl(i);
            assert_eq!(most_significant_bit(x), i);
        }
        for i in 2u8..=255 {
            let x = U256::from(1).shl(i).sub(U256::from(1));
            assert_eq!(most_significant_bit(x), i - 1);
        }
        assert_eq!(most_significant_bit(U256::MAX), 255);
    }

    #[test]
    fn test_least_significant_bit() {
        for i in 1u8..=255 {
            let x = U256::from(1).shl(i);
            assert_eq!(least_significant_bit(x), i);
        }
        for i in 2u8..=255 {
            let x = U256::from(1).shl(i).sub(U256::from(1));
            assert_eq!(least_significant_bit(x), 0);
        }
        assert_eq!(least_significant_bit(U256::MAX), 0);
    }
}
