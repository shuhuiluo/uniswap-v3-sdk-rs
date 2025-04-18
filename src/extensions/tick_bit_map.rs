//! ## Tick Bit Map
//! The [`TickBitMapProvider`] trait provides
//! [`TickBitMapProvider::next_initialized_tick_within_one_word`] for a tick bit map that implements
//! [`TickBitMapProvider::get_word`].

use crate::prelude::*;
use alloy_primitives::{aliases::I24, map::rustc_hash::FxHashMap, uint, U256};

pub type TickBitMap<I = I24> = FxHashMap<I, U256>;

/// Provides [`Self::next_initialized_tick_within_one_word`] for a tick bit map that implements
/// [`Self::get_word`]
pub trait TickBitMapProvider {
    type Index: TickIndex;

    /// Get a bitmap word at a specific index
    async fn get_word(&self, index: Self::Index) -> Result<U256, Error>;

    #[inline]
    async fn next_initialized_tick_within_one_word(
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
            let word = self.get_word(word_pos).await?;
            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let msb = if initialized {
                masked.most_significant_bit() as u8 as i32
            } else {
                0
            }
            .try_into()
            .unwrap();
            let next = ((word_pos << 8) + msb) * tick_spacing;
            Ok((next, initialized))
        } else {
            // start from the word of the next tick, since the current tick state doesn't matter
            let compressed = compressed + Self::Index::ONE;
            let (word_pos, bit_pos) = compressed.position();
            // all the 1s at or to the left of the `bit_pos`
            let mask = U256::ZERO - (uint!(1_U256) << bit_pos);
            let word = self.get_word(word_pos).await?;
            let masked = word & mask;
            let initialized = masked != U256::ZERO;
            let lsb = if initialized {
                masked.least_significant_bit() as u8 as i32
            } else {
                255
            }
            .try_into()
            .unwrap();
            let next = ((word_pos << 8) + lsb) * tick_spacing;
            Ok((next, initialized))
        }
    }
}

impl<I: TickIndex> TickBitMapProvider for TickBitMap<I> {
    type Index = I;

    #[inline]
    async fn get_word(&self, index: Self::Index) -> Result<U256, Error> {
        Ok(self.get(&index).copied().unwrap_or(U256::ZERO))
    }
}
