use crate::prelude::*;
use alloy_primitives::{aliases::I24, Signed};
use core::{
    fmt::Debug,
    hash::Hash,
    ops::{Add, BitAnd, Div, Mul, Rem, Shl, Shr, Sub},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick<I = i32> {
    pub index: I,
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
}

impl<I: TickIndex> Tick<I> {
    #[inline]
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

/// The trait for tick indexes used across [`Tick`], [`TickDataProvider`], and [`TickList`].
///
/// Implemented for [`i32`] and [`Signed`].
pub trait TickIndex:
    Copy
    + Debug
    + Default
    + Hash
    + Ord
    + BitAnd<Output = Self>
    + Add<Output = Self>
    + Div<Output = Self>
    + Mul<Output = Self>
    + Rem<Output = Self>
    + Sub<Output = Self>
    + Shl<i32, Output = Self>
    + Shr<i32, Output = Self>
    + TryFrom<i32, Error: Debug>
    + TryInto<i32, Error: Debug>
    + Send
    + Sync
{
    const ZERO: Self;
    const ONE: Self;

    #[inline]
    fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    fn from_i24(value: I24) -> Self;

    fn to_i24(self) -> I24;

    #[inline]
    fn compress(self, tick_spacing: Self) -> Self {
        assert!(tick_spacing > Self::ZERO, "TICK_SPACING");
        if self % tick_spacing < Self::ZERO {
            self / tick_spacing - Self::ONE
        } else {
            self / tick_spacing
        }
    }

    #[inline]
    fn position(self) -> (Self, u8) {
        (
            self >> 8,
            (self & Self::try_from(0xff).unwrap()).try_into().unwrap() as u8,
        )
    }
}

impl TickIndex for i32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;

    #[inline]
    fn from_i24(value: I24) -> Self {
        value.as_i32()
    }

    #[inline]
    fn to_i24(self) -> I24 {
        I24::try_from(self).unwrap()
    }
}

impl<const BITS: usize, const LIMBS: usize> TickIndex for Signed<BITS, LIMBS> {
    const ZERO: Self = Self::ZERO;
    const ONE: Self = Self::ONE;

    #[inline]
    fn from_i24(value: I24) -> Self {
        Self::try_from(value.as_i32()).unwrap()
    }

    #[inline]
    fn to_i24(self) -> I24 {
        I24::try_from(self.as_i32()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::I32;

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

    #[test]
    fn test_tick_index_i32() {
        assert_eq!(i32::from_i24(MIN_TICK), MIN_TICK_I32);
        assert_eq!(i32::from_i24(MAX_TICK), MAX_TICK_I32);
        assert_eq!(MIN_TICK_I32.to_i24(), MIN_TICK);
        assert_eq!(MAX_TICK_I32.to_i24(), MAX_TICK);
    }

    #[test]
    fn test_tick_index_signed() {
        assert_eq!(
            I32::from_i24(MIN_TICK),
            I32::from_limbs([MIN_TICK_I32 as u32 as u64])
        );
        assert_eq!(
            I32::from_i24(MAX_TICK),
            I32::from_limbs([MAX_TICK_I32 as u64])
        );
        assert_eq!(I32::from_i24(MIN_TICK).to_i24(), MIN_TICK);
        assert_eq!(I32::from_i24(MAX_TICK).to_i24(), MAX_TICK);
    }

    #[test]
    fn test_compress() {
        assert_eq!(42.compress(60), 0);
        assert_eq!(
            I24::try_from(42)
                .unwrap()
                .compress(I24::try_from(60).unwrap()),
            I24::try_from(42.compress(60)).unwrap()
        );
        assert_eq!((-42).compress(60), -1);
        assert_eq!(
            I24::try_from(-42)
                .unwrap()
                .compress(I24::try_from(60).unwrap()),
            I24::try_from((-42).compress(60)).unwrap()
        );
        assert_eq!(42.compress(10), 4);
        assert_eq!(
            I24::try_from(42)
                .unwrap()
                .compress(I24::try_from(10).unwrap()),
            I24::try_from(42.compress(10)).unwrap()
        );
        assert_eq!((-42).compress(10), -5);
        assert_eq!(
            I24::try_from(-42)
                .unwrap()
                .compress(I24::try_from(10).unwrap()),
            I24::try_from((-42).compress(10)).unwrap()
        );
    }
}
