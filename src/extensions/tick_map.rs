//! ## Tick Map
//! [`TickMapTrait`] is a trait that provides a way to access tick data directly from a hashmap,
//! supposedly more efficient than [`TickList`].

use crate::prelude::*;
use alloy_primitives::U256;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TickMap {
    pub bitmap: HashMap<i16, U256>,
    pub ticks: HashMap<i32, Tick>,
}

pub trait TickMapTrait {
    type Tick;

    fn position(tick: i32) -> (i16, u8) {
        ((tick >> 8) as i16, (tick & 0xff) as u8)
    }

    fn get_bitmap(&self, tick: i32) -> Result<U256>;

    fn get_tick(&self, index: i32) -> &Self::Tick;
}

// impl TickMapTrait for TickMap {}

pub trait TickMapDataProvider: TickMapTrait + TickDataProvider {}
