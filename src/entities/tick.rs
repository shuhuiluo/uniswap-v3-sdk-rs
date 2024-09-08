use crate::prelude::*;
use alloy_primitives::aliases::I24;
use core::{
    fmt::Debug,
    ops::{Add, Div, Mul, Rem, Shl, Shr, Sub},
};
use num_integer::Integer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick<I = i32> {
    pub index: I,
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
}

pub trait TickIndex:
    Copy
    + Debug
    + Default
    + Ord
    + Add<Output = Self>
    + Div<Output = Self>
    + Mul<Output = Self>
    + Rem<Output = Self>
    + Sub<Output = Self>
    + Shl<i32, Output = Self>
    + Shr<i32, Output = Self>
    + TryFrom<i32, Error: Debug>
    + TryInto<i32, Error: Debug>
{
    fn zero() -> Self;

    fn one() -> Self;

    fn is_zero(self) -> bool {
        self == Self::zero()
    }

    fn from_i24(value: I24) -> Self;

    fn to_i24(self) -> I24;

    fn div_floor(self, other: Self) -> Self;
}

impl TickIndex for i32 {
    #[inline]
    fn zero() -> Self {
        0
    }

    #[inline]
    fn one() -> Self {
        1
    }

    fn from_i24(value: I24) -> Self {
        value.as_i32()
    }

    fn to_i24(self) -> I24 {
        I24::try_from(self).unwrap()
    }

    #[inline]
    fn div_floor(self, other: Self) -> Self {
        Integer::div_floor(&self, &other)
    }
}

impl<I: TickIndex> Tick<I> {
    pub fn new(index: I, liquidity_gross: u128, liquidity_net: i128) -> Self {
        assert!(
            index >= I::from_i24(MIN_TICK) && index <= I::from_i24(MAX_TICK),
            "TICK"
        );
        Self {
            index,
            liquidity_gross,
            liquidity_net,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "TICK")]
    fn test_tick_below_min_tick() {
        Tick::new(MIN_TICK_I32 - 1, 0, 0);
    }

    #[test]
    #[should_panic(expected = "TICK")]
    fn test_tick_above_max_tick() {
        Tick::new(MAX_TICK_I32 + 1, 0, 0);
    }
}
