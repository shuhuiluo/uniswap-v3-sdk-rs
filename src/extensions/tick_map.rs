//! ## Tick Map
//! [`TickMap`] provides a way to access tick data directly from a hashmap, supposedly more
//! efficient than [`TickList`].

use crate::prelude::*;
use alloy::uint;
use alloy_primitives::{aliases::I24, U256};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug)]
pub struct TickMap<I = I24> {
    pub bitmap: FxHashMap<I, U256>,
    pub inner: FxHashMap<I, Tick<I>>,
    pub tick_spacing: I,
}

impl<I: TickIndex> TickMap<I> {
    #[inline]
    #[must_use]
    pub fn new(ticks: Vec<Tick<I>>, tick_spacing: I) -> Self {
        ticks.validate_list(tick_spacing);
        let mut bitmap = FxHashMap::<I, U256>::default();
        for tick in &ticks {
            let compressed = tick.index.compress(tick_spacing);
            let (word_pos, bit_pos) = compressed.position();
            let word = bitmap.get(&word_pos).unwrap_or(&U256::ZERO);
            bitmap.insert(word_pos, word | uint!(1_U256) << bit_pos);
        }
        Self {
            bitmap,
            inner: FxHashMap::from_iter(ticks.into_iter().map(|tick| (tick.index, tick))),
            tick_spacing,
        }
    }
}

impl<I: TickIndex> TickDataProvider for TickMap<I> {
    type Index = I;

    #[inline]
    fn get_tick(&self, tick: Self::Index) -> Result<&Tick<Self::Index>, Error> {
        self.inner
            .get(&tick)
            .ok_or(Error::InvalidTick(tick.to_i24()))
    }

    #[inline]
    fn next_initialized_tick_within_one_word(
        &self,
        tick: Self::Index,
        lte: bool,
        tick_spacing: Self::Index,
    ) -> Result<(Self::Index, bool), Error> {
        let compressed = tick.compress(tick_spacing);
        if lte {
            let (word_pos, bit_pos) = compressed.position();
            // all the 1s at or to the right of the current `bit_pos`
            // (2 << bitPos) may overflow but fine since 2 << 255 = 0
            let mask = (TWO << bit_pos) - uint!(1_U256);
            let word = self.bitmap.get(&word_pos).unwrap_or(&U256::ZERO);
            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let bit_pos = if initialized {
                let msb = masked.most_significant_bit() as u8;
                (bit_pos - msb) as i32
            } else {
                bit_pos as i32
            };
            let next = (compressed - Self::Index::try_from(bit_pos).unwrap()) * tick_spacing;
            Ok((next, initialized))
        } else {
            // start from the word of the next tick, since the current tick state doesn't matter
            let compressed = compressed + Self::Index::ONE;
            let (word_pos, bit_pos) = compressed.position();
            // all the 1s at or to the left of the `bit_pos`
            let mask = U256::ZERO - (uint!(1_U256) << bit_pos);
            let word = self.bitmap.get(&word_pos).unwrap_or(&U256::ZERO);
            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let bit_pos = if initialized {
                let lsb = masked.least_significant_bit() as u8;
                (lsb - bit_pos) as i32
            } else {
                (255 - bit_pos) as i32
            };
            let next = (compressed + Self::Index::try_from(bit_pos).unwrap()) * tick_spacing;
            Ok((next, initialized))
        }
    }
}
