use alloy_primitives::{address, b256, Address, B256};

pub const FACTORY_ADDRESS: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");

pub const ADDRESS_ZERO: Address = Address::ZERO;

pub const POOL_INIT_CODE_HASH: B256 =
    b256!("e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54");

/// The default factory enabled fee amounts, denominated in hundredths of bips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeeAmount {
    LOWEST = 100,
    LOW = 500,
    MEDIUM = 3000,
    HIGH = 10000,
}

impl FeeAmount {
    /// The default factory tick spacings by fee amount.
    pub const fn tick_spacing(&self) -> i32 {
        match self {
            Self::LOWEST => 1,
            Self::LOW => 10,
            Self::MEDIUM => 60,
            Self::HIGH => 200,
        }
    }
}

impl From<u32> for FeeAmount {
    fn from(fee: u32) -> Self {
        match fee {
            100 => Self::LOWEST,
            500 => Self::LOW,
            3000 => Self::MEDIUM,
            10000 => Self::HIGH,
            _ => panic!("Invalid fee amount"),
        }
    }
}

impl From<i32> for FeeAmount {
    fn from(tick_spacing: i32) -> Self {
        match tick_spacing {
            1 => Self::LOWEST,
            10 => Self::LOW,
            60 => Self::MEDIUM,
            200 => Self::HIGH,
            _ => panic!("Invalid tick spacing"),
        }
    }
}
