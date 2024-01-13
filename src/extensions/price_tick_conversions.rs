//! ## Price and tick conversions
//! Utility functions for converting between [`i32`] ticks, [`BigDecimal`] prices, and SDK Core [`Price`] prices.
//! Ported from [uniswap-v3-automation-sdk](https://github.com/Aperture-Finance/uniswap-v3-automation-sdk/blob/8bc54456753f454848d25029631f4e64ff573e12/price.ts).

use crate::prelude::*;
use alloy_primitives::U256;
use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use uniswap_sdk_core::prelude::*;

static MIN_PRICE: Lazy<Fraction> = Lazy::new(|| {
    Fraction::new(
        u256_to_big_int(MIN_SQRT_RATIO).pow(2),
        u256_to_big_int(Q192),
    )
});
static MAX_PRICE: Lazy<Fraction> = Lazy::new(|| {
    Fraction::new(
        u256_to_big_int(MAX_SQRT_RATIO).pow(2) - u256_to_big_int(ONE),
        u256_to_big_int(Q192),
    )
});

/// Parses the specified price string for the price of `base_token` denominated in `quote_token`.
///
/// ## Arguments
///
/// * `base_token`: The base token.
/// * `quote_token`: The quote token.
/// * `price`: The amount of `quote_token` that is worth the same as 1 `base_token`.
///
/// ## Returns
///
/// The parsed price as an instance of [`Price`] in [`uniswap_sdk_core`].
///
/// ## Examples
///
/// ```
/// use uniswap_sdk_core::{prelude::Token, token};
/// use uniswap_v3_sdk::prelude::parse_price;
///
/// let price = parse_price(
///    token!(1, "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8, "WBTC"),
///    token!(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, "WETH"),
///    "10.23",
/// ).unwrap();
/// ```
pub fn parse_price<TBase, TQuote>(
    base_token: TBase,
    quote_token: TQuote,
    price: &str,
) -> Result<Price<TBase, TQuote>>
where
    TBase: CurrencyTrait,
    TQuote: CurrencyTrait,
{
    // Check whether `price` is a valid string of decimal number.
    // This regex matches any number of digits optionally followed by '.' which is then followed by at least one digit.
    let re = Regex::new(r"^\d*\.?\d+$").unwrap();
    if !re.is_match(price) {
        bail!("Invalid price string");
    }

    let (whole, fraction) = match price.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (price, ""),
    };
    let decimals = fraction.len();
    let without_decimals = BigInt::from_str(&format!("{}{}", whole, fraction))?;
    let numerator = without_decimals * BigInt::from(10).pow(quote_token.decimals() as u32);
    let denominator = BigInt::from(10).pow(decimals as u32 + base_token.decimals() as u32);
    Ok(Price::new(base_token, quote_token, denominator, numerator))
}

/// Given a sqrt ratio, returns the price of the base token in terms of the quote token.
///
/// ## Arguments
///
/// * `sqrt_ratio_x96`: The sqrt ratio of the base token in terms of the quote token as a Q64.96 [`U256`].
/// * `base_token`: The base token.
/// * `quote_token`: The quote token.
///
/// ## Returns
///
/// The price of the base token in terms of the quote token as an instance of [`Price`] in [`uniswap_sdk_core`].
///
pub fn sqrt_ratio_x96_to_price(
    sqrt_ratio_x96: U256,
    base_token: Token,
    quote_token: Token,
) -> Result<Price<Token, Token>> {
    let ratio_x192 = u256_to_big_uint(sqrt_ratio_x96).pow(2);
    let q192 = u256_to_big_uint(Q192);
    Ok(if base_token.sorts_before(&quote_token)? {
        Price::new(base_token, quote_token, q192, ratio_x192)
    } else {
        Price::new(base_token, quote_token, ratio_x192, q192)
    })
}

/// Same as [`price_to_closest_tick`] but returns [`MIN_TICK`] or [`MAX_TICK`] if the price is outside Uniswap's range.
pub fn price_to_closest_tick_safe(price: &Price<Token, Token>) -> Result<i32> {
    let sorted = price
        .meta
        .base_currency
        .sorts_before(&price.meta.quote_currency)?;
    if price.as_fraction() < *MIN_PRICE {
        Ok(if sorted { MIN_TICK } else { MAX_TICK })
    } else if price.as_fraction() > *MAX_PRICE {
        Ok(if sorted { MAX_TICK } else { MIN_TICK })
    } else {
        price_to_closest_tick(price)
    }
}

/// Finds the closest usable tick for the specified price and pool fee tier.
///
/// ## Arguments
///
/// * `price`: The price of two tokens in the liquidity pool. Either token0 or token1 may be the base token.
/// * `fee`: The liquidity pool fee tier.
///
/// ## Returns
///
/// The closest usable tick.
///
pub fn price_to_closest_usable_tick(price: &Price<Token, Token>, fee: FeeAmount) -> Result<i32> {
    Ok(nearest_usable_tick(
        price_to_closest_tick_safe(price)?,
        fee.tick_spacing(),
    ))
}

/// Given a tick, returns the price of token0 in terms of token1 as a [`BigDecimal`].
///
/// ## Arguments
///
/// * `tick`: The tick for which to return the price.
///
pub fn tick_to_big_price(tick: i32) -> Result<BigDecimal> {
    let sqrt_ratio_x96 = get_sqrt_ratio_at_tick(tick)?;
    Ok(BigDecimal::from(u256_to_big_int(sqrt_ratio_x96).pow(2)) / u256_to_big_decimal(Q192))
}

/// Convert a [`FractionBase`] object to a [`BigDecimal`].
pub fn fraction_to_big_decimal<M>(price: &impl FractionBase<M>) -> BigDecimal {
    price.to_decimal()
}

/// Given a price ratio of token1/token0, calculate the sqrt ratio of token1/token0.
///
/// ## Arguments
///
/// * `price`: The price ratio of token1/token0, as a [`BigDecimal`].
///
/// ## Returns
///
/// The sqrt ratio of token1/token0, as a [`U256`].
///
pub fn price_to_sqrt_ratio_x96(price: &BigDecimal) -> U256 {
    if price < &BigDecimal::zero() {
        panic!("Invalid price: must be non-negative");
    }
    let price_x192 = price * u256_to_big_decimal(Q192);
    let sqrt_ratio_x96 = price_x192.to_bigint().unwrap().sqrt();
    if sqrt_ratio_x96 < u256_to_big_int(MIN_SQRT_RATIO) {
        MIN_SQRT_RATIO
    } else if sqrt_ratio_x96 >= u256_to_big_int(MAX_SQRT_RATIO) {
        MAX_SQRT_RATIO - ONE
    } else {
        big_int_to_u256(sqrt_ratio_x96)
    }
}

#[cfg(test)]
mod tests {
    // TODO: Add tests.
}
