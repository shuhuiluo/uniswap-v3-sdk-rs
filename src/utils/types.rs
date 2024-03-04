use alloy_primitives::{Address, I256, U256};
use bigdecimal::BigDecimal;
use ethers_core::types;
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{Signed, ToBytes};
use std::ops::Neg;

pub trait ToAlloy {
    type AlloyType;

    fn to_alloy(self) -> Self::AlloyType;
}

pub trait ToEthers {
    type EthersType;

    fn to_ethers(self) -> Self::EthersType;
}

impl ToAlloy for types::U256 {
    type AlloyType = U256;

    fn to_alloy(self) -> Self::AlloyType {
        U256::from_limbs(self.0)
    }
}

impl ToEthers for U256 {
    type EthersType = types::U256;

    fn to_ethers(self) -> Self::EthersType {
        types::U256(self.into_limbs())
    }
}

impl ToAlloy for types::Address {
    type AlloyType = Address;

    fn to_alloy(self) -> Self::AlloyType {
        self.to_fixed_bytes().into()
    }
}

impl ToEthers for Address {
    type EthersType = types::Address;

    fn to_ethers(self) -> Self::EthersType {
        self.into_array().into()
    }
}

pub fn u256_to_big_uint(x: U256) -> BigUint {
    BigUint::from_bytes_be(&x.to_be_bytes::<32>())
}

pub fn u256_to_big_int(x: U256) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, &x.to_be_bytes::<32>())
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
    U256::from_be_slice(&x.to_be_bytes())
}

pub fn big_int_to_i256(x: BigInt) -> I256 {
    if x.is_positive() {
        I256::from_raw(big_int_to_u256(x))
    } else {
        I256::from_raw(big_int_to_u256(x.neg())).neg()
    }
}

pub const fn u128_to_uint256(x: u128) -> U256 {
    U256::from_limbs([x as u64, (x >> 64) as u64, 0, 0])
}

pub fn u256_to_big_decimal(x: U256) -> BigDecimal {
    BigDecimal::from(u256_to_big_int(x))
}
