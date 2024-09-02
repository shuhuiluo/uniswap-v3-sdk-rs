use alloy_primitives::{I256, U160, U256};
use bigdecimal::BigDecimal;
use core::ops::Neg;
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{Signed, ToBytes};

pub fn u256_to_big_uint(x: U256) -> BigUint {
    BigUint::from_bytes_be(&x.to_be_bytes::<32>())
}

pub fn u160_to_big_uint(x: U160) -> BigUint {
    BigUint::from_bytes_be(&x.to_be_bytes::<20>())
}

pub fn u256_to_big_int(x: U256) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, &x.to_be_bytes::<32>())
}

pub fn u160_to_big_int(x: U160) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, &x.to_be_bytes::<20>())
}

pub fn i256_to_big_int(x: I256) -> BigInt {
    if x.is_positive() {
        u256_to_big_int(x.into_raw())
    } else {
        u256_to_big_int(x.neg().into_raw()).neg()
    }
}

pub fn big_uint_to_u256(x: BigUint) -> U256 {
    U256::from_be_slice(&x.to_be_bytes())
}

pub fn big_int_to_u256(x: BigInt) -> U256 {
    U256::from_le_slice(&x.to_le_bytes())
}

pub fn big_int_to_u160(x: BigInt) -> U160 {
    U160::from_le_slice(&x.to_le_bytes())
}

pub fn big_int_to_i256(x: BigInt) -> I256 {
    if x.is_positive() {
        I256::from_raw(big_int_to_u256(x))
    } else {
        I256::from_raw(big_int_to_u256(x.neg())).neg()
    }
}

pub const fn u128_to_u256(x: u128) -> U256 {
    U256::from_limbs([x as u64, (x >> 64) as u64, 0, 0])
}

pub const fn u160_to_u256(x: U160) -> U256 {
    let limbs = x.into_limbs();
    U256::from_limbs([limbs[0], limbs[1], limbs[2], 0])
}

pub const fn u256_to_u160_unchecked(x: U256) -> U160 {
    let limbs = x.into_limbs();
    U160::from_limbs([limbs[0], limbs[1], limbs[2]])
}

pub fn u256_to_big_decimal(x: U256) -> BigDecimal {
    BigDecimal::from(u256_to_big_int(x))
}

pub fn u160_to_big_decimal(x: U160) -> BigDecimal {
    BigDecimal::from(u160_to_big_int(x))
}
