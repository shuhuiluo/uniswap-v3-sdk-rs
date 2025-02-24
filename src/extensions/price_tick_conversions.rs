//! ## Price and tick conversions
//! Utility functions for converting between [`I24`] ticks, [`BigDecimal`] prices, and SDK Core
//! [`Price`] prices. Ported from [uniswap-v3-automation-sdk](https://github.com/Aperture-Finance/uniswap-v3-automation-sdk/blob/8bc54456753f454848d25029631f4e64ff573e12/price.ts).

use crate::prelude::{Error, *};
use alloy_primitives::{aliases::I24, U160};
use fastnum::dec512;
use num_integer::Roots;
use once_cell::sync::Lazy;
use uniswap_sdk_core::prelude::*;

pub static MIN_PRICE: Lazy<Fraction> =
    Lazy::new(|| Fraction::new(MIN_SQRT_RATIO.to_big_int().pow(2), Q192_BIG_INT));
pub static MAX_PRICE: Lazy<Fraction> = Lazy::new(|| {
    Fraction::new(
        MAX_SQRT_RATIO.to_big_int().pow(2) - ONE.to_big_int(),
        Q192_BIG_INT,
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
/// use uniswap_sdk_core::{prelude::*, token};
/// use uniswap_v3_sdk::prelude::parse_price;
///
/// let price = parse_price(
///     token!(1, "2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8, "WBTC"),
///     token!(1, "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, "WETH"),
///     "10.23",
/// )
/// .unwrap();
/// ```
#[cfg(feature = "parse_price")]
#[inline]
pub fn parse_price<TBase, TQuote>(
    base_token: TBase,
    quote_token: TQuote,
    price: &str,
) -> anyhow::Result<Price<TBase, TQuote>>
where
    TBase: BaseCurrency,
    TQuote: BaseCurrency,
{
    // Check whether `price` is a valid string of decimal number.
    // This regex matches any number of digits optionally followed by '.' which is then followed by
    // at least one digit.
    let re = regex::Regex::new(r"^\d*\.?\d+$")?;
    if !re.is_match(price) {
        anyhow::bail!("Invalid price string");
    }

    let (whole, fraction) = match price.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (price, ""),
    };
    let decimals = fraction.len();
    let without_decimals =
        <BigInt as core::str::FromStr>::from_str(&alloc::format!("{}{}", whole, fraction))
            .map_err(|e| anyhow::anyhow!("Invalid price string: {}", e))?;
    let numerator = without_decimals * BigInt::from(10).pow(quote_token.decimals() as u32);
    let denominator = BigInt::from(10).pow(decimals as u32 + base_token.decimals() as u32);
    Ok(Price::new(base_token, quote_token, denominator, numerator))
}

/// Given a sqrt ratio, returns the price of the base token in terms of the quote token.
///
/// ## Arguments
///
/// * `sqrt_ratio_x96`: The sqrt ratio of the base token in terms of the quote token as a Q64.96
///   [`U160`].
/// * `base_token`: The base token.
/// * `quote_token`: The quote token.
///
/// ## Returns
///
/// The price of the base token in terms of the quote token as an instance of [`Price`] in
/// [`uniswap_sdk_core`].
///
/// ## Examples
///
/// ```
/// use uniswap_sdk_core::{prelude::*, token};
/// use uniswap_v3_sdk::prelude::*;
///
/// let token0 = token!(1, "2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8, "WBTC");
/// let token1 = token!(1, "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, "WETH");
/// let min_price = tick_to_price(token0.clone(), token1.clone(), MIN_TICK).unwrap();
/// assert_eq!(
///     sqrt_ratio_x96_to_price(MIN_SQRT_RATIO, token0, token1).unwrap(),
///     min_price
/// );
/// ```
#[inline]
pub fn sqrt_ratio_x96_to_price(
    sqrt_ratio_x96: U160,
    base_token: Token,
    quote_token: Token,
) -> Result<Price<Token, Token>, Error> {
    let ratio_x192 = sqrt_ratio_x96.to_big_int().pow(2);
    Ok(if base_token.sorts_before(&quote_token)? {
        Price::new(base_token, quote_token, Q192_BIG_INT, ratio_x192)
    } else {
        Price::new(base_token, quote_token, ratio_x192, Q192_BIG_INT)
    })
}

/// Same as [`price_to_closest_tick`] but returns [`MIN_TICK`] or [`MAX_TICK`] if the price is
/// outside Uniswap's range.
#[inline]
pub fn price_to_closest_tick_safe(price: &Price<Token, Token>) -> Result<I24, Error> {
    let sorted = price.base_currency.sorts_before(&price.quote_currency)?;
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
/// * `price`: The price of two tokens in the liquidity pool. Either token0 or token1 may be the
///   base token.
/// * `fee`: The liquidity pool fee tier.
///
/// ## Returns
///
/// The closest usable tick.
///
/// ## Examples
///
/// ```
/// use uniswap_sdk_core::{prelude::*, token};
/// use uniswap_v3_sdk::prelude::*;
///
/// let token0 = token!(1, "2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 8, "WBTC");
/// let token1 = token!(1, "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, "WETH");
/// let fee = FeeAmount::MEDIUM;
/// let min_price = Price::new(
///     token0.clone(),
///     token1.clone(),
///     MIN_PRICE.denominator(),
///     MIN_PRICE.numerator(),
/// );
/// let max_price = Price::new(
///     token0.clone(),
///     token1.clone(),
///     MAX_PRICE.denominator(),
///     MAX_PRICE.numerator(),
/// );
///
/// assert_eq!(
///     price_to_closest_usable_tick(&min_price, fee).unwrap(),
///     nearest_usable_tick(MIN_TICK, fee.tick_spacing())
/// );
/// assert_eq!(
///     price_to_closest_usable_tick(&min_price.invert(), fee).unwrap(),
///     nearest_usable_tick(MIN_TICK, fee.tick_spacing())
/// );
/// assert_eq!(
///     price_to_closest_usable_tick(&max_price.invert(), fee).unwrap(),
///     nearest_usable_tick(MAX_TICK, fee.tick_spacing())
/// );
/// ```
#[inline]
pub fn price_to_closest_usable_tick(
    price: &Price<Token, Token>,
    fee: FeeAmount,
) -> Result<I24, Error> {
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
/// ## Examples
///
/// ```
/// use alloy_primitives::aliases::I24;
/// use num_traits::{FromPrimitive, Pow, ToPrimitive};
/// use uniswap_sdk_core::prelude::BigDecimal;
/// use uniswap_v3_sdk::prelude::*;
///
/// assert!(
///     (tick_to_big_price(I24::from_limbs([100])).unwrap()
///         - BigDecimal::from(1.0001f64.pow(100i32)))
///     .abs()
///         < BigDecimal::from(1e-14)
/// );
/// ```
#[inline]
pub fn tick_to_big_price(tick: I24) -> Result<BigDecimal, Error> {
    let sqrt_ratio_x96 = get_sqrt_ratio_at_tick(tick)?;
    Ok(sqrt_ratio_x96.to_big_int().pow(2).to_big_decimal() / Q192.to_big_decimal())
}

/// Convert a [`FractionBase`] object to a [`BigDecimal`].
#[inline]
pub fn fraction_to_big_decimal<M, F>(price: &F) -> BigDecimal
where
    M: Clone,
    F: FractionBase<M>,
{
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
/// The sqrt ratio of token1/token0, as a [`U160`].
///
/// ## Examples
///
/// ```
/// use uniswap_v3_sdk::prelude::*;
///
/// let price = tick_to_big_price(MAX_TICK).unwrap();
/// assert_eq!(price_to_sqrt_ratio_x96(price), MAX_SQRT_RATIO);
/// ```
#[inline]
#[must_use]
pub fn price_to_sqrt_ratio_x96(price: BigDecimal) -> U160 {
    assert!(!price.is_negative(), "Invalid price: must be non-negative");
    let price_x192 = price * Q192.to_big_decimal();
    let sqrt_ratio_x96 = U160::from_big_uint(price_x192.to_big_uint().sqrt());
    if sqrt_ratio_x96 < MIN_SQRT_RATIO {
        MIN_SQRT_RATIO
    } else if sqrt_ratio_x96 > MAX_SQRT_RATIO {
        MAX_SQRT_RATIO
    } else {
        sqrt_ratio_x96
    }
}

/// For a given tick range from `tick_lower` to `tick_upper`, and a given proportion of the position
/// value that is held in token0, calculate the price of token0 denominated in token1.
///
/// ## Arguments
///
/// * `token0_ratio`: The proportion of the position value that is held in token0, as a
///   [`BigDecimal`] between 0 and 1, inclusive.
/// * `tick_lower`: The lower tick of the range.
/// * `tick_upper`: The upper tick of the range.
///
/// ## Returns
///
/// The price of token0 denominated in token1 for the specified tick range and token0 value
/// proportion.
#[inline]
pub fn token0_ratio_to_price(
    token0_ratio: BigDecimal,
    tick_lower: I24,
    tick_upper: I24,
) -> Result<BigDecimal, Error> {
    const ONE: BigDecimal = dec512!(1);
    assert!(
        tick_upper > tick_lower,
        "Invalid tick range: tickUpper must be greater than tickLower"
    );
    assert!(
        !(token0_ratio.is_negative() || token0_ratio > ONE),
        "Invalid token0ValueProportion: must be a value between 0 and 1, inclusive"
    );
    if token0_ratio.is_zero() {
        return tick_to_big_price(tick_upper);
    }
    if token0_ratio == ONE {
        return tick_to_big_price(tick_lower);
    }
    let sqrt_ratio_lower_x96 = get_sqrt_ratio_at_tick(tick_lower)?;
    let sqrt_ratio_upper_x96 = get_sqrt_ratio_at_tick(tick_upper)?;
    let l = sqrt_ratio_lower_x96.to_big_decimal() / Q96.to_big_decimal();
    let u = sqrt_ratio_upper_x96.to_big_decimal() / Q96.to_big_decimal();
    let r = token0_ratio;
    let a = r - ONE;
    let b = u * (ONE - dec512!(2) * r);
    let c = r * l * u;
    let numerator = b + (b * b - dec512!(4) * a * c).sqrt();
    let denominator = dec512!(-2) * a;
    let sqrt_price = numerator / denominator;
    Ok(sqrt_price * sqrt_price)
}

/// Given a price ratio of token1/token0, calculate the proportion of the position value that is
/// held in token0 for a given tick range. Inverse of [`token0_ratio_to_price`].
///
/// ## Arguments
///
/// * `price`: The price ratio of token1/token0, as a [`BigDecimal`].
/// * `tick_lower`: The lower tick of the range.
/// * `tick_upper`: The upper tick of the range.
///
/// ## Returns
///
/// The proportion of the position value that is held in token0, as a [`BigDecimal`] between 0 and
/// 1, inclusive.
#[inline]
pub fn token0_price_to_ratio(
    price: BigDecimal,
    tick_lower: I24,
    tick_upper: I24,
) -> Result<BigDecimal, Error> {
    if tick_upper <= tick_lower {
        return Err(Error::InvalidRange);
    }
    let sqrt_price_x96 = price_to_sqrt_ratio_x96(price);
    let tick = sqrt_price_x96.get_tick_at_sqrt_ratio()?;
    // only token0
    if tick < tick_lower {
        Ok(dec512!(1))
    }
    // only token1
    else if tick >= tick_upper {
        Ok(BigDecimal::ZERO)
    } else {
        let liquidity = 2_u128 << 96;
        let amount0 = get_amount_0_delta(
            sqrt_price_x96,
            get_sqrt_ratio_at_tick(tick_upper)?,
            liquidity,
            false,
        )?;
        let amount1 = get_amount_1_delta(
            get_sqrt_ratio_at_tick(tick_lower)?,
            sqrt_price_x96,
            liquidity,
            false,
        )?;
        let value0 = amount0.to_big_decimal() * price;
        Ok(value0 / (value0 + amount1.to_big_decimal()))
    }
}

/// Returns the tick range for a position ratio and range width.
///
/// ## Arguments
///
/// * `width`: The width of the range.
/// * `tick_current`: The current tick of the pool.
/// * `token0_ratio`: The proportion of the position value that is held in token0, as a
///   [`BigDecimal`] number between 0 and 1, inclusive.
///
/// ## Returns
///
/// The tick range as a tuple of `(tick_lower, tick_upper)`.
///
/// ## Examples
///
/// ```
/// use alloy_primitives::aliases::I24;
/// use fastnum::dec512;
/// use num_traits::real::Real;
/// use uniswap_sdk_core::prelude::*;
/// use uniswap_v3_sdk::prelude::*;
///
/// let tick_current = I24::from_limbs([200000]);
/// let price = tick_to_big_price(tick_current).unwrap();
/// let token0_ratio = "0.3".parse::<BigDecimal>().unwrap();
/// let width = I24::from_limbs([1000]);
/// let (tick_lower, tick_upper) =
///     tick_range_from_width_and_ratio(width, tick_current, token0_ratio).unwrap();
/// assert_eq!(tick_upper - tick_lower, width);
/// let price_lower_sqrt = tick_to_big_price(tick_lower).unwrap().sqrt();
/// let price_upper_sqrt = tick_to_big_price(tick_upper).unwrap().sqrt();
/// const ONE: BigDecimal = dec512!(1);
/// let amount0 = ONE / price.sqrt() - ONE / price_upper_sqrt;
/// let amount1 = price.sqrt() - price_lower_sqrt;
/// let value0 = amount0 * price;
/// let ratio = value0 / (value0 + amount1);
/// assert!((ratio - token0_ratio).abs() < "0.001".parse::<BigDecimal>().unwrap());
/// ```
#[inline]
pub fn tick_range_from_width_and_ratio(
    width: I24,
    tick_current: I24,
    token0_ratio: BigDecimal,
) -> Result<(I24, I24), Error> {
    const ONE: BigDecimal = dec512!(1);
    const TWO: BigDecimal = dec512!(2);
    assert!(
        !(token0_ratio.is_negative() || token0_ratio > ONE),
        "Invalid token0ValueProportion: must be a value between 0 and 1, inclusive"
    );
    let (tick_lower, tick_upper) = if token0_ratio.is_zero() {
        (tick_current - width, tick_current)
    } else if token0_ratio == ONE {
        (tick_current, tick_current + width)
    } else {
        let price = tick_to_big_price(tick_current)?;
        let a = token0_ratio;
        let b = (ONE - a * TWO) * price.sqrt();
        let c = price * (a - ONE) / tick_to_big_price(width)?.sqrt();
        let price_lower_sqrt = ((b * b - a * c * dec512!(4)).sqrt() - b) / (a * TWO);
        let sqrt_ratio_lower_x96 = price_lower_sqrt * Q96.to_big_decimal();
        let tick_lower =
            U160::from_big_uint(sqrt_ratio_lower_x96.to_big_uint()).get_tick_at_sqrt_ratio()?;
        (tick_lower, tick_lower + width)
    };
    Ok((tick_lower, tick_upper))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token0_ratio_to_price_conversion() {
        let tick_lower = I24::from_limbs([253320]);
        let tick_upper = I24::from_limbs([264600]);
        assert_eq!(
            token0_ratio_to_price(dec512!(0), tick_lower, tick_upper).unwrap(),
            tick_to_big_price(tick_upper).unwrap()
        );
        assert_eq!(
            token0_ratio_to_price(dec512!(1), tick_lower, tick_upper).unwrap(),
            tick_to_big_price(tick_lower).unwrap()
        );
        let price = token0_ratio_to_price(dec512!(0.3), tick_lower, tick_upper).unwrap();
        assert_eq!(
            price
                .with_rounding_mode(RoundingMode::HalfUp)
                .round(30)
                .to_string(),
            "226996287752.678057810335753063814266625941"
        );
        let token0_ratio = token0_price_to_ratio(price, tick_lower, tick_upper).unwrap();
        assert_eq!(
            token0_ratio
                .with_rounding_mode(RoundingMode::HalfUp)
                .round(30)
                .to_string(),
            "0.299999999999999999999998780740"
        );
    }
}
