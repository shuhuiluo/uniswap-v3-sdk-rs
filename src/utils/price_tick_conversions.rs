use super::{encode_sqrt_ratio_x96, get_sqrt_ratio_at_tick, get_tick_at_sqrt_ratio, Q192};
use alloy_primitives::U256;
use anyhow::Result;
use num_bigint::BigUint;
use num_traits::ToBytes;
use uniswap_sdk_core_rust::entities::{
    fractions::{fraction::FractionTrait, price::Price},
    token::Token,
};

/// Returns a price object corresponding to the input tick and the base/quote token.
/// Inputs must be tokens because the address order is used to interpret the price represented by the tick.
///
/// # Arguments
///
/// * `base_token`: the base token of the price
/// * `quote_token`: the quote token of the price
/// * `tick`: the tick for which to return the price
///
pub fn tick_to_price(
    base_token: Token,
    quote_token: Token,
    tick: i32,
) -> Result<Price<Token, Token>> {
    let sqrt_ratio_x96 = get_sqrt_ratio_at_tick(tick)?;
    let ratio_x192 = sqrt_ratio_x96 * sqrt_ratio_x96;
    let q192 = BigUint::from_radix_be(&Q192.to_be_bytes::<32>(), 16).unwrap();
    let ratio_x192 = BigUint::from_radix_be(&ratio_x192.to_be_bytes::<32>(), 16).unwrap();
    Ok(if base_token.sorts_before(&quote_token) {
        Price::new(base_token, quote_token, q192, ratio_x192)
    } else {
        Price::new(base_token, quote_token, ratio_x192, q192)
    })
}

/// Returns the first tick for which the given price is greater than or equal to the tick price
///
/// # Arguments
///
/// * `price`: for which to return the closest tick that represents a price less than or equal to
/// the input price, i.e. the price of the returned tick is less than or equal to the input price
///
pub fn price_to_closest_tick(price: Price<Token, Token>) -> Result<i32> {
    let sorted = price
        .meta
        .base_currency
        .sorts_before(&price.meta.quote_currency);
    let sqrt_ratio_x96 = if sorted {
        encode_sqrt_ratio_x96(price.numerator().clone(), price.denominator().clone())
    } else {
        encode_sqrt_ratio_x96(price.denominator().clone(), price.numerator().clone())
    };
    let tick = get_tick_at_sqrt_ratio(U256::from_le_slice(&sqrt_ratio_x96.to_le_bytes()))?;
    let next_tick_price = tick_to_price(
        price.meta.base_currency.clone(),
        price.meta.quote_currency.clone(),
        tick + 1,
    )?;
    Ok(if sorted {
        if !price.less_than(&next_tick_price) {
            tick + 1
        } else {
            tick
        }
    } else if !price.greater_than(&next_tick_price) {
        tick + 1
    } else {
        tick
    })
}
