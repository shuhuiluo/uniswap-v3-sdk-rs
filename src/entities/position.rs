use crate::prelude::*;
use alloy_primitives::U256;
use anyhow::Result;
use std::{cmp::PartialEq, fmt};
use uniswap_sdk_core::prelude::*;

/// Represents a position on a Uniswap V3 Pool
#[derive(Clone)]
pub struct Position<P> {
    pub pool: Pool<P>,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: u128,
    _token0_amount: Option<CurrencyAmount<Token>>,
    _token1_amount: Option<CurrencyAmount<Token>>,
    _mint_amounts: Option<MintAmounts>,
}

impl<P> fmt::Debug for Position<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position")
            .field("pool", &self.pool)
            .field("tick_lower", &self.tick_lower)
            .field("tick_upper", &self.tick_upper)
            .field("liquidity", &self.liquidity)
            .finish()
    }
}

impl<P> PartialEq for Position<P> {
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool
            && self.tick_lower == other.tick_lower
            && self.tick_upper == other.tick_upper
            && self.liquidity == other.liquidity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintAmounts {
    pub amount0: U256,
    pub amount1: U256,
}

impl<P> Position<P> {
    /// Constructs a position for a given pool with the given liquidity
    ///
    /// ## Arguments
    ///
    /// * `pool`: For which pool the liquidity is assigned
    /// * `liquidity`: The amount of liquidity that is in the position
    /// * `tick_lower`: The lower tick of the position
    /// * `tick_upper`: The upper tick of the position
    pub fn new(pool: Pool<P>, liquidity: u128, tick_lower: i32, tick_upper: i32) -> Self {
        assert!(tick_lower < tick_upper, "TICK_ORDER");
        assert!(
            tick_lower >= MIN_TICK && tick_lower % pool.tick_spacing() == 0,
            "TICK_LOWER"
        );
        assert!(
            tick_upper <= MAX_TICK && tick_upper % pool.tick_spacing() == 0,
            "TICK_UPPER"
        );
        Self {
            pool,
            liquidity,
            tick_lower,
            tick_upper,
            _token0_amount: None,
            _token1_amount: None,
            _mint_amounts: None,
        }
    }

    /// Returns the price of token0 at the lower tick
    pub fn token0_price_lower(&self) -> Result<Price<Token, Token>> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_lower,
        )
    }

    /// Returns the price of token0 at the upper tick
    pub fn token0_price_upper(&self) -> Result<Price<Token, Token>> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_upper,
        )
    }

    /// Returns the amount of token0 that this position's liquidity could be burned for at the
    /// current pool price
    pub fn amount0(&mut self) -> Result<CurrencyAmount<Token>> {
        if self._token0_amount.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    u256_to_big_int(get_amount_0_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        false,
                    )?),
                )?)
            } else if self.pool.tick_current < self.tick_upper {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    u256_to_big_int(get_amount_0_delta(
                        self.pool.sqrt_ratio_x96,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        false,
                    )?),
                )?)
            } else {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    BigInt::zero(),
                )?)
            }
        }
        Ok(self._token0_amount.clone().unwrap())
    }

    /// Returns the amount of token1 that this position's liquidity could be burned for at the
    /// current pool price
    pub fn amount1(&mut self) -> Result<CurrencyAmount<Token>> {
        if self._token1_amount.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    BigInt::zero(),
                )?)
            } else if self.pool.tick_current < self.tick_upper {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    u256_to_big_int(get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        self.pool.sqrt_ratio_x96,
                        self.liquidity,
                        false,
                    )?),
                )?)
            } else {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    u256_to_big_int(get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        false,
                    )?),
                )?)
            }
        }
        Ok(self._token1_amount.clone().unwrap())
    }

    /// Returns the lower and upper sqrt ratios if the price 'slips' up to slippage tolerance
    /// percentage
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: The amount by which the price can 'slip' before the transaction will
    ///   revert
    ///
    /// ## Returns
    ///
    /// (sqrt_ratio_x96_lower, sqrt_ratio_x96_upper)
    fn ratios_after_slippage(&self, slippage_tolerance: &Percent) -> (U256, U256) {
        let one = Percent::new(1, 1);
        let price_lower = self.pool.token0_price().as_fraction()
            * ((one.clone() - slippage_tolerance.clone()).as_fraction());
        let price_upper = self.pool.token0_price().as_fraction()
            * ((one + slippage_tolerance.clone()).as_fraction());

        let mut sqrt_ratio_x96_lower =
            encode_sqrt_ratio_x96(price_lower.numerator(), price_lower.denominator());
        if sqrt_ratio_x96_lower <= MIN_SQRT_RATIO {
            sqrt_ratio_x96_lower = MIN_SQRT_RATIO + ONE;
        }

        let mut sqrt_ratio_x96_upper =
            encode_sqrt_ratio_x96(price_upper.numerator(), price_upper.denominator());
        if sqrt_ratio_x96_upper >= MAX_SQRT_RATIO {
            sqrt_ratio_x96_upper = MAX_SQRT_RATIO - ONE;
        }

        (sqrt_ratio_x96_lower, sqrt_ratio_x96_upper)
    }

    /// Returns the minimum amounts that must be sent in order to safely mint the amount of
    /// liquidity held by the position
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: Tolerance of unfavorable slippage from the current price
    ///
    /// ## Returns
    ///
    /// The amounts, with slippage
    pub fn mint_amounts_with_slippage(
        &mut self,
        slippage_tolerance: &Percent,
    ) -> Result<MintAmounts> {
        // Get lower/upper prices
        let (sqrt_ratio_x96_lower, sqrt_ratio_x96_upper) =
            self.ratios_after_slippage(slippage_tolerance);

        // Construct counterfactual pools
        let pool_lower = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_lower,
            0, // liquidity doesn't matter
        )?;
        let pool_upper = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_upper,
            0, // liquidity doesn't matter
        )?;

        // Because the router is imprecise, we need to calculate the position that will be created
        // (assuming no slippage)
        let MintAmounts { amount0, amount1 } = self.mint_amounts()?;
        let position_that_will_be_created = Position::from_amounts(
            Pool::new(
                self.pool.token0.clone(),
                self.pool.token1.clone(),
                self.pool.fee,
                self.pool.sqrt_ratio_x96,
                self.pool.liquidity,
            )?,
            self.tick_lower,
            self.tick_upper,
            amount0,
            amount1,
            false,
        )?;

        // We want the smaller amounts...
        // ...which occurs at the upper price for amount0...
        let amount0 = Position::new(
            pool_upper,
            position_that_will_be_created.liquidity,
            self.tick_lower,
            self.tick_upper,
        )
        .mint_amounts()?
        .amount0;
        // ...and the lower for amount1
        let amount1 = Position::new(
            pool_lower,
            position_that_will_be_created.liquidity,
            self.tick_lower,
            self.tick_upper,
        )
        .mint_amounts()?
        .amount1;

        Ok(MintAmounts { amount0, amount1 })
    }

    /// Returns the minimum amounts that should be requested in order to safely burn the amount of
    /// liquidity held by the position with the given slippage tolerance
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: tolerance of unfavorable slippage from the current price
    ///
    /// ## Returns
    ///
    /// The amounts, with slippage
    pub fn burn_amounts_with_slippage(&self, slippage_tolerance: &Percent) -> Result<(U256, U256)> {
        // get lower/upper prices
        let (sqrt_ratio_x96_lower, sqrt_ratio_x96_upper) =
            self.ratios_after_slippage(slippage_tolerance);

        // construct counterfactual pools
        let pool_lower = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_lower,
            0, // liquidity doesn't matter
        )?;
        let pool_upper = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_upper,
            0, // liquidity doesn't matter
        )?;

        // we want the smaller amounts...
        // ...which occurs at the upper price for amount0...
        let amount0 = Position::new(pool_upper, self.liquidity, self.tick_lower, self.tick_upper)
            .amount0()?
            .quotient();
        // ...and the lower for amount1
        let amount1 = Position::new(pool_lower, self.liquidity, self.tick_lower, self.tick_upper)
            .amount1()?
            .quotient();

        Ok((big_int_to_u256(amount0), big_int_to_u256(amount1)))
    }

    /// Returns the minimum amounts that must be sent in order to mint the amount of liquidity held
    /// by the position at the current price for the pool
    pub fn mint_amounts(&mut self) -> Result<MintAmounts> {
        if self._mint_amounts.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._mint_amounts = Some(MintAmounts {
                    amount0: get_amount_0_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        true,
                    )?,
                    amount1: U256::ZERO,
                })
            } else if self.pool.tick_current < self.tick_upper {
                self._mint_amounts = Some(MintAmounts {
                    amount0: get_amount_0_delta(
                        self.pool.sqrt_ratio_x96,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        true,
                    )?,
                    amount1: get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        self.pool.sqrt_ratio_x96,
                        self.liquidity,
                        true,
                    )?,
                })
            } else {
                self._mint_amounts = Some(MintAmounts {
                    amount0: U256::ZERO,
                    amount1: get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower)?,
                        get_sqrt_ratio_at_tick(self.tick_upper)?,
                        self.liquidity,
                        true,
                    )?,
                })
            }
        }
        Ok(self._mint_amounts.unwrap())
    }

    /// Computes the maximum amount of liquidity received for a given amount of token0, token1,
    /// and the prices at the tick boundaries.
    ///
    /// ## Arguments
    ///
    /// * `pool`: The pool for which the position should be created
    /// * `tick_lower`: The lower tick of the position
    /// * `tick_upper`: The upper tick of the position
    /// * `amount0`: token0 amount
    /// * `amount1`: token1 amount
    /// * `use_full_precision`: If false, liquidity will be maximized according to what the router
    ///   can calculate, not what core can theoretically support
    ///
    /// ## Returns
    ///
    /// The position with the maximum amount of liquidity received
    pub fn from_amounts(
        pool: Pool<P>,
        tick_lower: i32,
        tick_upper: i32,
        amount0: U256,
        amount1: U256,
        use_full_precision: bool,
    ) -> Result<Self> {
        let sqrt_ratio_a_x96 = get_sqrt_ratio_at_tick(tick_lower)?;
        let sqrt_ratio_b_x96 = get_sqrt_ratio_at_tick(tick_upper)?;
        let liquidity = max_liquidity_for_amounts(
            pool.sqrt_ratio_x96,
            sqrt_ratio_a_x96,
            sqrt_ratio_b_x96,
            amount0,
            amount1,
            use_full_precision,
        );
        Ok(Self::new(
            pool,
            liquidity.to_u128().unwrap(),
            tick_lower,
            tick_upper,
        ))
    }

    /// Computes a position with the maximum amount of liquidity received for a given amount of
    /// token0, assuming an unlimited amount of token1
    ///
    /// ## Arguments
    ///
    /// * `pool`: The pool for which the position is created
    /// * `tick_lower`: The lower tick
    /// * `tick_upper`: The upper tick
    /// * `amount0`: The desired amount of token0
    /// * `use_full_precision`: If true, liquidity will be maximized according to what the router
    ///   can calculate, not what core can theoretically support
    pub fn from_amount0(
        pool: Pool<P>,
        tick_lower: i32,
        tick_upper: i32,
        amount0: U256,
        use_full_precision: bool,
    ) -> Result<Self> {
        Self::from_amounts(
            pool,
            tick_lower,
            tick_upper,
            amount0,
            U256::MAX,
            use_full_precision,
        )
    }

    /// Computes a position with the maximum amount of liquidity received for a given amount of
    /// token1, assuming an unlimited amount of token0
    ///
    /// ## Arguments
    ///
    /// * `pool`: The pool for which the position is created
    /// * `tick_lower`: The lower tick
    /// * `tick_upper`: The upper tick
    /// * `amount1`: The desired amount of token1
    pub fn from_amount1(
        pool: Pool<P>,
        tick_lower: i32,
        tick_upper: i32,
        amount1: U256,
    ) -> Result<Self> {
        // this function always uses full precision
        Self::from_amounts(pool, tick_lower, tick_upper, U256::MAX, amount1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::uint;
    use once_cell::sync::Lazy;

    static POOL_SQRT_RATIO_START: Lazy<U256> =
        Lazy::new(|| encode_sqrt_ratio_x96(BigInt::from(10).pow(8), BigInt::from(10).pow(20)));
    static POOL_TICK_CURRENT: Lazy<i32> =
        Lazy::new(|| get_tick_at_sqrt_ratio(*POOL_SQRT_RATIO_START).unwrap());
    const TICK_SPACING: i32 = FeeAmount::LOW.tick_spacing();

    static DAI_USDC_POOL: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
        Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            *POOL_SQRT_RATIO_START,
            0,
        )
        .unwrap()
    });

    #[test]
    fn can_be_constructed_around_0_tick() {
        let position = Position::new(DAI_USDC_POOL.clone(), 1, -10, 10);
        assert_eq!(position.liquidity, 1);
    }

    #[test]
    fn can_use_min_and_max_ticks() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            1,
            nearest_usable_tick(MIN_TICK, TICK_SPACING),
            nearest_usable_tick(MAX_TICK, TICK_SPACING),
        );
        assert_eq!(position.liquidity, 1);
    }

    #[test]
    #[should_panic(expected = "TICK_ORDER")]
    fn tick_lower_must_be_less_than_tick_upper() {
        Position::new(DAI_USDC_POOL.clone(), 1, 10, -10);
    }

    #[test]
    #[should_panic(expected = "TICK_ORDER")]
    fn tick_lower_cannot_equal_tick_upper() {
        Position::new(DAI_USDC_POOL.clone(), 1, -10, -10);
    }

    #[test]
    #[should_panic(expected = "TICK_LOWER")]
    fn tick_lower_must_be_multiple_of_tick_spacing() {
        Position::new(DAI_USDC_POOL.clone(), 1, -5, 10);
    }

    #[test]
    #[should_panic(expected = "TICK_LOWER")]
    fn tick_lower_must_be_greater_than_min_tick() {
        Position::new(
            DAI_USDC_POOL.clone(),
            1,
            nearest_usable_tick(MIN_TICK, TICK_SPACING) - TICK_SPACING,
            10,
        );
    }

    #[test]
    #[should_panic(expected = "TICK_UPPER")]
    fn tick_upper_must_be_multiple_of_tick_spacing() {
        Position::new(DAI_USDC_POOL.clone(), 1, -10, 15);
    }

    #[test]
    #[should_panic(expected = "TICK_UPPER")]
    fn tick_upper_must_be_less_than_max_tick() {
        Position::new(
            DAI_USDC_POOL.clone(),
            1,
            -10,
            nearest_usable_tick(MAX_TICK, TICK_SPACING) + TICK_SPACING,
        );
    }

    #[test]
    fn amount0_is_correct_for_price_above() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e12 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        assert_eq!(
            position.amount0().unwrap().quotient().to_string(),
            "49949961958869841"
        );
    }

    #[test]
    fn amount0_is_correct_for_price_below() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        assert_eq!(position.amount0().unwrap().quotient().to_string(), "0");
    }

    #[test]
    fn amount0_is_correct_for_in_range_position() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        assert_eq!(
            position.amount0().unwrap().quotient().to_string(),
            "120054069145287995769396"
        );
    }

    #[test]
    fn amount1_is_correct_for_price_above() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        assert_eq!(position.amount1().unwrap().quotient().to_string(), "0");
    }

    #[test]
    fn amount1_is_correct_for_price_below() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        assert_eq!(
            position.amount1().unwrap().quotient().to_string(),
            "49970077052"
        );
    }

    #[test]
    fn amount1_is_correct_for_in_range_position() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        assert_eq!(
            position.amount1().unwrap().quotient().to_string(),
            "79831926242"
        );
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_below() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841738198");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_above() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077053");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_within() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "120054069145287995740584");
        assert_eq!(amount1.to_string(), "79831926243");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_below_05_percent_slippage() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841738198");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_above_05_percent_slippage() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077053");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_positions_within_05_percent_slippage() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "95063440240746211432007");
        assert_eq!(amount1.to_string(), "54828800461");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_pool_at_min_price() {
        let position = Position::new(
            Pool::new(DAI.clone(), USDC.clone(), FeeAmount::LOW, MIN_SQRT_RATIO, 0).unwrap(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 100);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841754181");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_pool_at_max_price() {
        let position = Position::new(
            Pool::new(
                DAI.clone(),
                USDC.clone(),
                FeeAmount::LOW,
                MAX_SQRT_RATIO - uint!(1_U256),
                0,
            )
            .unwrap(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 100);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "50045084659");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_below() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841754181");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_above() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077052");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_within() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(0, 1);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "120054069145287995769396");
        assert_eq!(amount1.to_string(), "79831926242");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_below_05_percent_slippage() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841754181");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_above_05_percent_slippage() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077052");
    }

    #[test]
    fn burn_amounts_with_slippage_is_correct_for_positions_within_05_percent_slippage() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 10000);
        let (amount0, amount1) = position
            .burn_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "95063440240746211454822");
        assert_eq!(amount1.to_string(), "54828800460");
    }

    #[test]
    fn mint_amounts_is_correct_for_pool_at_min_price() {
        let mut position = Position::new(
            Pool::new(DAI.clone(), USDC.clone(), FeeAmount::LOW, MIN_SQRT_RATIO, 0).unwrap(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 100);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841738198");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn mint_amounts_with_slippage_is_correct_for_pool_at_max_price() {
        let mut position = Position::new(
            Pool::new(
                DAI.clone(),
                USDC.clone(),
                FeeAmount::LOW,
                MAX_SQRT_RATIO - uint!(1_U256),
                0,
            )
            .unwrap(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let slippage_tolerance = Percent::new(5, 100);
        let MintAmounts { amount0, amount1 } = position
            .mint_amounts_with_slippage(&slippage_tolerance)
            .unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "50045084660");
    }

    #[test]
    fn mint_amounts_is_correct_for_positions_above() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841754182");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn mint_amounts_is_correct_for_positions_below() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING,
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077053");
    }

    #[test]
    fn mint_amounts_is_correct_for_positions_within() {
        let mut position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * 2,
            nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * 2,
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "120054069145287995769397");
        assert_eq!(amount1.to_string(), "79831926243");
    }
}
