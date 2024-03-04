//! ## Price and tick conversions
//! Utility functions for converting between [`i32`] ticks and SDK Core [`Price`] prices.

use crate::prelude::*;
use anyhow::Result;
use uniswap_sdk_core::prelude::*;

/// Returns a price object corresponding to the input tick and the base/quote token.
/// Inputs must be tokens because the address order is used to interpret the price represented by
/// the tick.
///
/// ## Arguments
///
/// * `base_token`: the base token of the price
/// * `quote_token`: the quote token of the price
/// * `tick`: the tick for which to return the price
pub fn tick_to_price(
    base_token: Token,
    quote_token: Token,
    tick: i32,
) -> Result<Price<Token, Token>> {
    let sqrt_ratio_x96 = get_sqrt_ratio_at_tick(tick)?;
    let ratio_x192 = u256_to_big_uint(sqrt_ratio_x96).pow(2);
    let q192 = u256_to_big_uint(Q192);
    Ok(if base_token.sorts_before(&quote_token)? {
        Price::new(base_token, quote_token, q192, ratio_x192)
    } else {
        Price::new(base_token, quote_token, ratio_x192, q192)
    })
}

/// Returns the first tick for which the given price is greater than or equal to the tick price
///
/// ## Arguments
///
/// * `price`: for which to return the closest tick that represents a price less than or equal to
/// the input price, i.e. the price of the returned tick is less than or equal to the input price
pub fn price_to_closest_tick(price: &Price<Token, Token>) -> Result<i32> {
    let sorted = price.base_currency.sorts_before(&price.quote_currency)?;
    let sqrt_ratio_x96 = if sorted {
        encode_sqrt_ratio_x96(price.numerator(), price.denominator())
    } else {
        encode_sqrt_ratio_x96(price.denominator(), price.numerator())
    };
    let tick = get_tick_at_sqrt_ratio(sqrt_ratio_x96)?;
    let next_tick_price = tick_to_price(
        price.base_currency.clone(),
        price.quote_currency.clone(),
        tick + 1,
    )?;
    Ok(if sorted {
        if price >= &next_tick_price {
            tick + 1
        } else {
            tick
        }
    } else if price <= &next_tick_price {
        tick + 1
    } else {
        tick
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use uniswap_sdk_core::token;

    static TOKEN0: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0000000000000000000000000000000000000000",
            18,
            "T0",
            "token0"
        )
    });
    static TOKEN1: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "1111111111111111111111111111111111111111",
            18,
            "T1",
            "token1"
        )
    });
    static TOKEN2_6DECIMALS: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "2222222222222222222222222222222222222222",
            6,
            "T2",
            "token2"
        )
    });

    #[test]
    fn tick_to_price_test_1() {
        assert_eq!(
            tick_to_price(TOKEN1.clone(), TOKEN0.clone(), -74959)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "1800"
        );
    }

    #[test]
    fn tick_to_price_test_2() {
        assert_eq!(
            tick_to_price(TOKEN0.clone(), TOKEN1.clone(), -74959)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "0.00055556"
        );
    }

    #[test]
    fn tick_to_price_test_3() {
        assert_eq!(
            tick_to_price(TOKEN0.clone(), TOKEN1.clone(), 74959)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "1800"
        );
    }

    #[test]
    fn tick_to_price_test_4() {
        assert_eq!(
            tick_to_price(TOKEN1.clone(), TOKEN0.clone(), 74959)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "0.00055556"
        );
    }

    #[test]
    fn tick_to_price_test_5() {
        assert_eq!(
            tick_to_price(TOKEN0.clone(), TOKEN2_6DECIMALS.clone(), -276225)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "1.01"
        );
    }

    #[test]
    fn tick_to_price_test_6() {
        assert_eq!(
            tick_to_price(TOKEN2_6DECIMALS.clone(), TOKEN0.clone(), -276225)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "0.99015"
        );
    }

    #[test]
    fn tick_to_price_test_7() {
        assert_eq!(
            tick_to_price(TOKEN0.clone(), TOKEN2_6DECIMALS.clone(), -276423)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "0.99015"
        );
    }

    #[test]
    fn tick_to_price_test_8() {
        assert_eq!(
            tick_to_price(TOKEN2_6DECIMALS.clone(), TOKEN0.clone(), -276423)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "1.0099"
        );
    }

    #[test]
    fn tick_to_price_test_9() {
        assert_eq!(
            tick_to_price(TOKEN0.clone(), TOKEN2_6DECIMALS.clone(), -276225)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "1.01"
        );
    }

    #[test]
    fn tick_to_price_test_10() {
        assert_eq!(
            tick_to_price(TOKEN2_6DECIMALS.clone(), TOKEN0.clone(), -276225)
                .unwrap()
                .to_significant(5, Rounding::RoundHalfUp)
                .unwrap(),
            "0.99015"
        );
    }

    #[test]
    fn price_to_closest_tick_test_1() {
        assert_eq!(
            price_to_closest_tick(&Price::new(TOKEN1.clone(), TOKEN0.clone(), 1, 1800)).unwrap(),
            -74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_2() {
        assert_eq!(
            price_to_closest_tick(&Price::new(TOKEN0.clone(), TOKEN1.clone(), 1800, 1)).unwrap(),
            -74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_3() {
        assert_eq!(
            price_to_closest_tick(&Price::new(
                TOKEN0.clone(),
                TOKEN2_6DECIMALS.clone(),
                BigInt::from(100) * BigInt::from(10).pow(18),
                BigInt::from(101) * BigInt::from(10).pow(6),
            ))
            .unwrap(),
            -276225
        );
    }

    #[test]
    fn price_to_closest_tick_test_4() {
        assert_eq!(
            price_to_closest_tick(&Price::new(
                TOKEN2_6DECIMALS.clone(),
                TOKEN0.clone(),
                BigInt::from(101) * BigInt::from(10).pow(6),
                BigInt::from(100) * BigInt::from(10).pow(18),
            ))
            .unwrap(),
            -276225
        );
    }

    #[test]
    fn price_to_closest_tick_test_5() {
        assert_eq!(
            price_to_closest_tick(&tick_to_price(TOKEN1.clone(), TOKEN0.clone(), -74960).unwrap())
                .unwrap(),
            -74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_6() {
        assert_eq!(
            price_to_closest_tick(&tick_to_price(TOKEN1.clone(), TOKEN0.clone(), 74960).unwrap())
                .unwrap(),
            74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_7() {
        assert_eq!(
            price_to_closest_tick(&tick_to_price(TOKEN0.clone(), TOKEN1.clone(), -74960).unwrap())
                .unwrap(),
            -74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_8() {
        assert_eq!(
            price_to_closest_tick(&tick_to_price(TOKEN0.clone(), TOKEN1.clone(), 74960).unwrap())
                .unwrap(),
            74960
        );
    }

    #[test]
    fn price_to_closest_tick_test_9() {
        assert_eq!(
            price_to_closest_tick(
                &tick_to_price(TOKEN0.clone(), TOKEN2_6DECIMALS.clone(), -276225).unwrap(),
            )
            .unwrap(),
            -276225
        );
    }

    #[test]
    fn price_to_closest_tick_test_10() {
        assert_eq!(
            price_to_closest_tick(
                &tick_to_price(TOKEN2_6DECIMALS.clone(), TOKEN0.clone(), -276225).unwrap(),
            )
            .unwrap(),
            -276225
        );
    }
}
