#![allow(non_camel_case_types)]

use alloy_primitives::{
    address,
    aliases::{I24, U24},
    b256, Address, B256,
};

pub const FACTORY_ADDRESS: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");

pub const POOL_INIT_CODE_HASH: B256 =
    b256!("e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54");

/// The default factory enabled fee amounts, denominated in hundredths of bips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeeAmount {
    LOWEST = 100,
    LOW_200 = 200,
    LOW_300 = 300,
    LOW_400 = 400,
    LOW = 500,
    MEDIUM = 3000,
    HIGH = 10000,
}

impl FeeAmount {
    /// The default factory tick spacings by fee amount.
    #[inline]
    #[must_use]
    pub const fn tick_spacing(&self) -> I24 {
        match self {
            Self::LOWEST => I24::ONE,
            Self::LOW_200 => I24::from_limbs([4]),
            Self::LOW_300 => I24::from_limbs([6]),
            Self::LOW_400 => I24::from_limbs([8]),
            Self::LOW => I24::from_limbs([10]),
            Self::MEDIUM => I24::from_limbs([60]),
            Self::HIGH => I24::from_limbs([200]),
        }
    }
}

impl From<u32> for FeeAmount {
    #[inline]
    fn from(fee: u32) -> Self {
        match fee {
            100 => Self::LOWEST,
            200 => Self::LOW_200,
            300 => Self::LOW_300,
            400 => Self::LOW_400,
            500 => Self::LOW,
            3000 => Self::MEDIUM,
            10000 => Self::HIGH,
            _ => panic!("Invalid fee amount"),
        }
    }
}

impl From<i32> for FeeAmount {
    #[inline]
    fn from(tick_spacing: i32) -> Self {
        match tick_spacing {
            1 => Self::LOWEST,
            4 => Self::LOW_200,
            6 => Self::LOW_300,
            8 => Self::LOW_400,
            10 => Self::LOW,
            60 => Self::MEDIUM,
            200 => Self::HIGH,
            _ => panic!("Invalid tick spacing"),
        }
    }
}

impl From<FeeAmount> for U24 {
    #[inline]
    fn from(fee: FeeAmount) -> Self {
        Self::from_limbs([fee as u64])
    }
}

impl From<U24> for FeeAmount {
    #[inline]
    fn from(fee: U24) -> Self {
        (fee.into_limbs()[0] as u32).into()
    }
}
