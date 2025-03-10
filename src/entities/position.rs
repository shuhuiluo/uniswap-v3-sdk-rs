use crate::prelude::{Error, *};
use alloy_primitives::{U160, U256};
use num_traits::ToPrimitive;
use uniswap_sdk_core::prelude::*;

/// Represents a position on a Uniswap V3 Pool
#[derive(Clone, Debug)]
pub struct Position<TP = NoTickDataProvider>
where
    TP: TickDataProvider,
{
    pub pool: Pool<TP>,
    pub tick_lower: TP::Index,
    pub tick_upper: TP::Index,
    pub liquidity: u128,
    _token0_amount: Option<CurrencyAmount<Token>>,
    _token1_amount: Option<CurrencyAmount<Token>>,
    _mint_amounts: Option<MintAmounts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintAmounts {
    pub amount0: U256,
    pub amount1: U256,
}

impl<TP> PartialEq for Position<TP>
where
    TP: TickDataProvider<Index: PartialEq>,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool
            && self.tick_lower == other.tick_lower
            && self.tick_upper == other.tick_upper
            && self.liquidity == other.liquidity
    }
}

impl<TP: TickDataProvider> Position<TP> {
    /// Constructs a position for a given pool with the given liquidity
    ///
    /// ## Arguments
    ///
    /// * `pool`: For which pool the liquidity is assigned
    /// * `liquidity`: The amount of liquidity that is in the position
    /// * `tick_lower`: The lower tick of the position
    /// * `tick_upper`: The upper tick of the position
    #[inline]
    pub fn new(
        pool: Pool<TP>,
        liquidity: u128,
        tick_lower: TP::Index,
        tick_upper: TP::Index,
    ) -> Self {
        assert!(tick_lower < tick_upper, "TICK_ORDER");
        assert!(
            tick_lower >= TP::Index::from_i24(MIN_TICK)
                && (tick_lower % pool.tick_spacing()).is_zero(),
            "TICK_LOWER"
        );
        assert!(
            tick_upper <= TP::Index::from_i24(MAX_TICK)
                && (tick_upper % pool.tick_spacing()).is_zero(),
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
    #[inline]
    pub fn token0_price_lower(&self) -> Result<Price<Token, Token>, Error> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_lower.to_i24(),
        )
    }

    /// Returns the price of token0 at the upper tick
    #[inline]
    pub fn token0_price_upper(&self) -> Result<Price<Token, Token>, Error> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_upper.to_i24(),
        )
    }

    /// Returns the amount of token0 that this position's liquidity could be burned for at the
    /// current pool price
    #[inline]
    pub fn amount0(&self) -> Result<CurrencyAmount<Token>, Error> {
        if self.pool.tick_current < self.tick_lower {
            CurrencyAmount::from_raw_amount(
                self.pool.token0.clone(),
                get_amount_0_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    false,
                )?
                .to_big_int(),
            )
        } else if self.pool.tick_current < self.tick_upper {
            CurrencyAmount::from_raw_amount(
                self.pool.token0.clone(),
                get_amount_0_delta(
                    self.pool.sqrt_ratio_x96,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    false,
                )?
                .to_big_int(),
            )
        } else {
            CurrencyAmount::from_raw_amount(self.pool.token0.clone(), BigInt::ZERO)
        }
        .map_err(Error::Core)
    }

    /// Returns the amount of token0 that this position's liquidity could be burned for at the
    /// current pool price
    #[inline]
    pub fn amount0_cached(&mut self) -> Result<CurrencyAmount<Token>, Error> {
        if let Some(amount) = &self._token0_amount {
            return Ok(amount.clone());
        }
        let amount = self.amount0()?;
        self._token0_amount = Some(amount.clone());
        Ok(amount)
    }

    /// Returns the amount of token1 that this position's liquidity could be burned for at the
    /// current pool price
    #[inline]
    pub fn amount1(&self) -> Result<CurrencyAmount<Token>, Error> {
        if self.pool.tick_current < self.tick_lower {
            CurrencyAmount::from_raw_amount(self.pool.token1.clone(), BigInt::ZERO)
        } else if self.pool.tick_current < self.tick_upper {
            CurrencyAmount::from_raw_amount(
                self.pool.token1.clone(),
                get_amount_1_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    self.pool.sqrt_ratio_x96,
                    self.liquidity,
                    false,
                )?
                .to_big_int(),
            )
        } else {
            CurrencyAmount::from_raw_amount(
                self.pool.token1.clone(),
                get_amount_1_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    false,
                )?
                .to_big_int(),
            )
        }
        .map_err(Error::Core)
    }

    /// Returns the amount of token1 that this position's liquidity could be burned for at the
    /// current pool price
    #[inline]
    pub fn amount1_cached(&mut self) -> Result<CurrencyAmount<Token>, Error> {
        if let Some(amount) = &self._token1_amount {
            return Ok(amount.clone());
        }
        let amount = self.amount1()?;
        self._token1_amount = Some(amount.clone());
        Ok(amount)
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
    fn ratios_after_slippage(&self, slippage_tolerance: &Percent) -> (U160, U160) {
        let one = Percent::new(1, 1);
        let token0_price = self.pool.token0_price().as_fraction();
        let price_lower = (one.clone() - slippage_tolerance).as_fraction() * &token0_price;
        let price_upper = token0_price * ((one + slippage_tolerance).as_fraction());

        let mut sqrt_ratio_x96_lower =
            encode_sqrt_ratio_x96(price_lower.numerator, price_lower.denominator);
        if sqrt_ratio_x96_lower <= MIN_SQRT_RATIO {
            sqrt_ratio_x96_lower = MIN_SQRT_RATIO + ONE;
        }

        let sqrt_ratio_x96_upper =
            if price_upper >= Fraction::new(MAX_SQRT_RATIO.to_big_int().pow(2), Q192_BIG_INT) {
                MAX_SQRT_RATIO - ONE
            } else {
                encode_sqrt_ratio_x96(price_upper.numerator, price_upper.denominator)
            };
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
    #[inline]
    pub fn mint_amounts_with_slippage(
        &mut self,
        slippage_tolerance: &Percent,
    ) -> Result<MintAmounts, Error> {
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
        let MintAmounts { amount0, amount1 } = self.mint_amounts_cached()?;
        let position_that_will_be_created = Position::from_amounts(
            Pool::new(
                self.pool.token0.clone(),
                self.pool.token1.clone(),
                self.pool.fee,
                self.pool.sqrt_ratio_x96,
                self.pool.liquidity,
            )?,
            self.tick_lower.try_into().unwrap(),
            self.tick_upper.try_into().unwrap(),
            amount0,
            amount1,
            false,
        )?;

        // We want the smaller amounts...
        // ...which occurs at the upper price for amount0...
        let amount0 = Position::new(
            pool_upper,
            position_that_will_be_created.liquidity,
            self.tick_lower.try_into().unwrap(),
            self.tick_upper.try_into().unwrap(),
        )
        .mint_amounts()?
        .amount0;
        // ...and the lower for amount1
        let amount1 = Position::new(
            pool_lower,
            position_that_will_be_created.liquidity,
            self.tick_lower.try_into().unwrap(),
            self.tick_upper.try_into().unwrap(),
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
    #[inline]
    pub fn burn_amounts_with_slippage(
        &self,
        slippage_tolerance: &Percent,
    ) -> Result<(U256, U256), Error> {
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
        let amount0 = Position::new(
            pool_upper,
            self.liquidity,
            self.tick_lower.try_into().unwrap(),
            self.tick_upper.try_into().unwrap(),
        )
        .amount0()?
        .quotient();
        // ...and the lower for amount1
        let amount1 = Position::new(
            pool_lower,
            self.liquidity,
            self.tick_lower.try_into().unwrap(),
            self.tick_upper.try_into().unwrap(),
        )
        .amount1()?
        .quotient();

        Ok((U256::from_big_int(amount0), U256::from_big_int(amount1)))
    }

    /// Returns the minimum amounts that must be sent in order to mint the amount of liquidity held
    /// by the position at the current price for the pool
    #[inline]
    pub fn mint_amounts(&self) -> Result<MintAmounts, Error> {
        Ok(if self.pool.tick_current < self.tick_lower {
            MintAmounts {
                amount0: get_amount_0_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    true,
                )?,
                amount1: U256::ZERO,
            }
        } else if self.pool.tick_current < self.tick_upper {
            MintAmounts {
                amount0: get_amount_0_delta(
                    self.pool.sqrt_ratio_x96,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    true,
                )?,
                amount1: get_amount_1_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    self.pool.sqrt_ratio_x96,
                    self.liquidity,
                    true,
                )?,
            }
        } else {
            MintAmounts {
                amount0: U256::ZERO,
                amount1: get_amount_1_delta(
                    get_sqrt_ratio_at_tick(self.tick_lower.to_i24())?,
                    get_sqrt_ratio_at_tick(self.tick_upper.to_i24())?,
                    self.liquidity,
                    true,
                )?,
            }
        })
    }

    /// Returns the minimum amounts that must be sent in order to mint the amount of liquidity held
    /// by the position at the current price for the pool
    #[inline]
    pub fn mint_amounts_cached(&mut self) -> Result<MintAmounts, Error> {
        if let Some(amounts) = &self._mint_amounts {
            return Ok(*amounts);
        }
        let amounts = self.mint_amounts()?;
        self._mint_amounts = Some(amounts);
        Ok(amounts)
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
    #[inline]
    pub fn from_amounts(
        pool: Pool<TP>,
        tick_lower: TP::Index,
        tick_upper: TP::Index,
        amount0: U256,
        amount1: U256,
        use_full_precision: bool,
    ) -> Result<Self, Error> {
        let sqrt_ratio_a_x96 = get_sqrt_ratio_at_tick(tick_lower.to_i24())?;
        let sqrt_ratio_b_x96 = get_sqrt_ratio_at_tick(tick_upper.to_i24())?;
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
    #[inline]
    pub fn from_amount0(
        pool: Pool<TP>,
        tick_lower: TP::Index,
        tick_upper: TP::Index,
        amount0: U256,
        use_full_precision: bool,
    ) -> Result<Self, Error> {
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
    #[inline]
    pub fn from_amount1(
        pool: Pool<TP>,
        tick_lower: TP::Index,
        tick_upper: TP::Index,
        amount1: U256,
    ) -> Result<Self, Error> {
        // this function always uses full precision
        Self::from_amounts(pool, tick_lower, tick_upper, U256::MAX, amount1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::aliases::I24;
    use once_cell::sync::Lazy;

    static POOL_SQRT_RATIO_START: Lazy<U160> =
        Lazy::new(|| encode_sqrt_ratio_x96(BigInt::from(10).pow(8), BigInt::from(10).pow(20)));
    static POOL_TICK_CURRENT: Lazy<I24> =
        Lazy::new(|| POOL_SQRT_RATIO_START.get_tick_at_sqrt_ratio().unwrap());
    const TICK_SPACING: I24 = I24::from_limbs([10]);

    static DAI_USDC_POOL: Lazy<Pool> = Lazy::new(|| {
        Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            *POOL_SQRT_RATIO_START,
            0,
        )
        .unwrap()
    });

    const TWO: I24 = I24::from_limbs([2]);

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
            nearest_usable_tick(MIN_TICK, TICK_SPACING).as_i32(),
            nearest_usable_tick(MAX_TICK, TICK_SPACING).as_i32(),
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
            (nearest_usable_tick(MIN_TICK, TICK_SPACING) - TICK_SPACING).as_i32(),
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
            (nearest_usable_tick(MAX_TICK, TICK_SPACING) + TICK_SPACING).as_i32(),
        );
    }

    #[test]
    fn amount0_is_correct_for_price_above() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e12 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        assert_eq!(
            position.amount0().unwrap().quotient().to_string(),
            "49949961958869841"
        );
    }

    #[test]
    fn amount0_is_correct_for_price_below() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
        );
        assert_eq!(position.amount0().unwrap().quotient().to_string(), "0");
    }

    #[test]
    fn amount0_is_correct_for_in_range_position() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        assert_eq!(
            position.amount0().unwrap().quotient().to_string(),
            "120054069145287995769396"
        );
    }

    #[test]
    fn amount1_is_correct_for_price_above() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        assert_eq!(position.amount1().unwrap().quotient().to_string(), "0");
    }

    #[test]
    fn amount1_is_correct_for_price_below() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
        );
        assert_eq!(
            position.amount1().unwrap().quotient().to_string(),
            "49970077052"
        );
    }

    #[test]
    fn amount1_is_correct_for_in_range_position() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
                MAX_SQRT_RATIO - ONE,
                0,
            )
            .unwrap(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let slippage_tolerance = Percent::default();
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
                MAX_SQRT_RATIO - ONE,
                0,
            )
            .unwrap(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
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
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "49949961958869841754182");
        assert_eq!(amount1.to_string(), "0");
    }

    #[test]
    fn mint_amounts_is_correct_for_positions_below() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING).as_i32(),
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "0");
        assert_eq!(amount1.to_string(), "49970077053");
    }

    #[test]
    fn mint_amounts_is_correct_for_positions_within() {
        let position = Position::new(
            DAI_USDC_POOL.clone(),
            100e18 as u128,
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) - TICK_SPACING * TWO).as_i32(),
            (nearest_usable_tick(*POOL_TICK_CURRENT, TICK_SPACING) + TICK_SPACING * TWO).as_i32(),
        );
        let MintAmounts { amount0, amount1 } = position.mint_amounts().unwrap();
        assert_eq!(amount0.to_string(), "120054069145287995769397");
        assert_eq!(amount1.to_string(), "79831926243");
    }
}
