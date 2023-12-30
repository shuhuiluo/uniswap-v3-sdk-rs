//! ## Bit Math Library in Rust
//! This library is a Rust port of the [BitMath library](https://github.com/uniswap/v3-core/blob/main/contracts/libraries/BitMath.sol) in Solidity,
//! with optimizations presented in [Solady](https://github.com/vectorized/solady/blob/main/src/utils/LibBit.sol).

use alloy_primitives::{b256, uint, B256, U256};
use std::ops::Sub;

pub fn most_significant_bit(x: U256) -> u8 {
    if x.is_zero() {
        panic!("ZERO")
    }
    // r = x >= 2**128 ? 128 : 0
    let mut r = (uint!(0xffffffffffffffffffffffffffffffff_U256).lt(&x) as u8) << 7;
    // r += (x >> r) >= 2**64 ? 64 : 0
    r |= (uint!(0xffffffffffffffff_U256).lt(&(x >> r)) as u8) << 6;
    // r += (x >> r) >= 2**32 ? 32 : 0
    r |= (uint!(0xffffffff_U256).lt(&(x >> r)) as u8) << 5;
    // r += (x >> r) >= 2**16 ? 16 : 0
    r |= (uint!(0xffff_U256).lt(&(x >> r)) as u8) << 4;
    // r += (x >> r) >= 2**8 ? 8 : 0
    r |= (uint!(0xff_U256).lt(&(x >> r)) as u8) << 3;

    const SEQUENCE: B256 =
        b256!("0706060506020504060203020504030106050205030304010505030400000000");
    let x_shr_r = (x >> r).into_limbs()[0] as u32;

    r | SEQUENCE[(0x8421084210842108cc6318c6db6d54be_u128
        .checked_shr(x_shr_r)
        .unwrap_or(0)
        & 0x1f) as usize]
}

pub fn least_significant_bit(mut x: U256) -> u8 {
    if x.is_zero() {
        panic!("ZERO")
    }
    // Isolate the least significant bit, x = x & -x = x & (~x + 1)
    x = x & U256::ZERO.sub(x);

    // r = x >= 2**128 ? 128 : 0
    let mut r = (uint!(0xffffffffffffffffffffffffffffffff_U256).lt(&x) as u8) << 7;
    // r += (x >> r) >= 2**64 ? 64 : 0
    r |= (uint!(0xffffffffffffffff_U256).lt(&(x >> r)) as u8) << 6;
    // r += (x >> r) >= 2**32 ? 32 : 0
    r |= (uint!(0xffffffff_U256).lt(&(x >> r)) as u8) << 5;

    // For the remaining 32 bits, use a De Bruijn lookup.
    // https://graphics.stanford.edu/~seander/bithacks.html#ZerosOnRightMultLookup
    const SEQUENCE: B256 =
        b256!("001f0d1e100c1d070f090b19131c1706010e11080a1a141802121b1503160405");
    let x_shr_r = (x >> r).into_limbs()[0] as u32;

    r | SEQUENCE[((0xd76453e0 / x_shr_r) & 0x1f) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Shl;

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
