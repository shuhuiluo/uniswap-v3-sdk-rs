use crate::prelude::*;
use alloy_primitives::U256;
use uniswap_sdk_core::prelude::*;

/// Represents a position on a Uniswap V3 Pool
#[derive(Clone)]
pub struct Position {
    pub pool: Pool,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: u128,
    _token0_amount: Option<CurrencyAmount<Token>>,
    _token1_amount: Option<CurrencyAmount<Token>>,
    _mint_amounts: Option<MintAmounts>,
}

#[derive(Clone)]
pub struct MintAmounts {
    pub amount0: U256,
    pub amount1: U256,
}

impl Position {
    /// Constructs a position for a given pool with the given liquidity
    ///
    /// # Arguments
    ///
    /// * `pool`: For which pool the liquidity is assigned
    /// * `liquidity`: The amount of liquidity that is in the position
    /// * `tick_lower`: The lower tick of the position
    /// * `tick_upper`: The upper tick of the position
    ///
    /// returns: Position
    ///
    pub const fn new(pool: Pool, liquidity: u128, tick_lower: i32, tick_upper: i32) -> Self {
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
    pub fn token0_price_lower(&self) -> Price<Token, Token> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_lower,
        )
        .unwrap()
    }

    /// Returns the price of token0 at the upper tick
    pub fn token0_price_upper(&self) -> Price<Token, Token> {
        tick_to_price(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.tick_upper,
        )
        .unwrap()
    }

    /// Returns the amount of token0 that this position's liquidity could be burned for at the current pool price
    pub fn amount0(&mut self) -> &CurrencyAmount<Token> {
        if self._token0_amount.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    u256_to_big_int(
                        get_amount_0_delta(
                            get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                            get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                            self.liquidity,
                            false,
                        )
                        .unwrap(),
                    ),
                ))
            } else if self.pool.tick_current < self.tick_upper {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    u256_to_big_int(
                        get_amount_0_delta(
                            self.pool.sqrt_ratio_x96,
                            get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                            self.liquidity,
                            false,
                        )
                        .unwrap(),
                    ),
                ))
            } else {
                self._token0_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token0.clone(),
                    BigInt::zero(),
                ))
            }
        }
        self._token0_amount.as_ref().unwrap()
    }

    /// Returns the amount of token1 that this position's liquidity could be burned for at the current pool price
    pub fn amount1(&mut self) -> &CurrencyAmount<Token> {
        if self._token1_amount.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    BigInt::zero(),
                ))
            } else if self.pool.tick_current < self.tick_upper {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    u256_to_big_int(
                        get_amount_1_delta(
                            get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                            self.pool.sqrt_ratio_x96,
                            self.liquidity,
                            false,
                        )
                        .unwrap(),
                    ),
                ))
            } else {
                self._token1_amount = Some(CurrencyAmount::from_raw_amount(
                    self.pool.token1.clone(),
                    u256_to_big_int(
                        get_amount_1_delta(
                            get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                            get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                            self.liquidity,
                            false,
                        )
                        .unwrap(),
                    ),
                ))
            }
        }
        self._token1_amount.as_ref().unwrap()
    }

    /// Returns the lower and upper sqrt ratios if the price 'slips' up to slippage tolerance percentage
    ///
    /// # Arguments
    ///
    /// * `slippage_tolerance`: The amount by which the price can 'slip' before the transaction will revert
    ///
    /// returns: The sqrt ratios after slippage
    ///
    fn ratios_after_slippage(&mut self, slippage_tolerance: &Percent) -> (U256, U256) {
        let one = Percent::new(1, 1);
        let price_lower = self
            .pool
            .token0_price()
            .as_fraction()
            .multiply(&one.subtract(slippage_tolerance).as_fraction());
        let price_upper = self
            .pool
            .token0_price()
            .as_fraction()
            .multiply(&one.add(slippage_tolerance).as_fraction());

        const ONE: U256 = U256::from_limbs([1, 0, 0, 0]);
        let mut sqrt_ratio_x96_lower = encode_sqrt_ratio_x96(
            price_lower.numerator().clone(),
            price_lower.denominator().clone(),
        );
        if sqrt_ratio_x96_lower <= MIN_SQRT_RATIO {
            sqrt_ratio_x96_lower = MIN_SQRT_RATIO + ONE;
        }

        let mut sqrt_ratio_x96_upper = encode_sqrt_ratio_x96(
            price_upper.numerator().clone(),
            price_upper.denominator().clone(),
        );
        if sqrt_ratio_x96_upper >= MAX_SQRT_RATIO {
            sqrt_ratio_x96_upper = MAX_SQRT_RATIO - ONE;
        }

        (sqrt_ratio_x96_lower, sqrt_ratio_x96_upper)
    }

    /// Returns the minimum amounts that must be sent in order to safely mint the amount of liquidity held by the position
    ///
    /// # Arguments
    ///
    /// * `slippage_tolerance`: Tolerance of unfavorable slippage from the current price
    ///
    /// returns: The amounts, with slippage
    ///
    pub fn mint_amounts_with_slippage(&mut self, slippage_tolerance: &Percent) -> MintAmounts {
        // Get lower/upper prices
        let (sqrt_ratio_x96_upper, sqrt_ratio_x96_lower) =
            self.ratios_after_slippage(slippage_tolerance);

        // Construct counterfactual pools
        let pool_lower = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_lower,
            0, // liquidity doesn't matter
        );
        let pool_upper = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_upper,
            0, // liquidity doesn't matter
        );

        // Because the router is imprecise, we need to calculate the position that will be created (assuming no slippage)
        let MintAmounts { amount0, amount1 } = self.mint_amounts();
        let position_that_will_be_created = Position::from_amounts(
            self.pool.clone(),
            self.tick_lower,
            self.tick_upper,
            amount0,
            amount1,
            false,
        );

        // We want the smaller amounts...
        // ...which occurs at the upper price for amount0...
        let amount0 = Position::new(
            pool_upper,
            position_that_will_be_created.liquidity,
            self.tick_lower,
            self.tick_upper,
        )
        .mint_amounts()
        .amount0;
        // ...and the lower for amount1
        let amount1 = Position::new(
            pool_lower,
            position_that_will_be_created.liquidity,
            self.tick_lower,
            self.tick_upper,
        )
        .mint_amounts()
        .amount1;

        MintAmounts { amount0, amount1 }
    }

    /// Returns the minimum amounts that should be requested in order to safely burn the amount of liquidity held by the
    /// position with the given slippage tolerance
    ///
    /// # Arguments
    ///
    /// * `slippage_tolerance`: tolerance of unfavorable slippage from the current price
    ///
    /// returns: The amounts, with slippage
    ///
    pub fn burn_amounts_with_slippage(&mut self, slippage_tolerance: &Percent) -> (U256, U256) {
        // get lower/upper prices
        let (sqrt_ratio_x96_upper, sqrt_ratio_x96_lower) =
            self.ratios_after_slippage(slippage_tolerance);

        // construct counterfactual pools
        let pool_lower = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_lower,
            0, // liquidity doesn't matter
        );
        let pool_upper = Pool::new(
            self.pool.token0.clone(),
            self.pool.token1.clone(),
            self.pool.fee,
            sqrt_ratio_x96_upper,
            0, // liquidity doesn't matter
        );

        // we want the smaller amounts...
        // ...which occurs at the upper price for amount0...
        let amount0 = Position::new(pool_upper, self.liquidity, self.tick_lower, self.tick_upper)
            .amount0()
            .quotient();
        // ...and the lower for amount1
        let amount1 = Position::new(pool_lower, self.liquidity, self.tick_lower, self.tick_upper)
            .amount1()
            .quotient();

        (big_int_to_u256(amount0), big_int_to_u256(amount1))
    }

    /// Returns the minimum amounts that must be sent in order to mint the amount of liquidity held by the position at
    /// the current price for the pool
    pub fn mint_amounts(&mut self) -> MintAmounts {
        if self._mint_amounts.is_none() {
            if self.pool.tick_current < self.tick_lower {
                self._mint_amounts = Some(MintAmounts {
                    amount0: get_amount_0_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                        get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                        self.liquidity,
                        true,
                    )
                    .unwrap(),
                    amount1: U256::ZERO,
                })
            } else if self.pool.tick_current < self.tick_upper {
                self._mint_amounts = Some(MintAmounts {
                    amount0: get_amount_0_delta(
                        self.pool.sqrt_ratio_x96,
                        get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                        self.liquidity,
                        true,
                    )
                    .unwrap(),
                    amount1: get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                        self.pool.sqrt_ratio_x96,
                        self.liquidity,
                        true,
                    )
                    .unwrap(),
                })
            } else {
                self._mint_amounts = Some(MintAmounts {
                    amount0: U256::ZERO,
                    amount1: get_amount_1_delta(
                        get_sqrt_ratio_at_tick(self.tick_lower).unwrap(),
                        get_sqrt_ratio_at_tick(self.tick_upper).unwrap(),
                        self.liquidity,
                        true,
                    )
                    .unwrap(),
                })
            }
        }
        self._mint_amounts.clone().unwrap()
    }

    /// Computes the maximum amount of liquidity received for a given amount of token0, token1,
    /// and the prices at the tick boundaries.
    ///
    /// # Arguments
    ///
    /// * `pool`: The pool for which the position should be created
    /// * `tick_lower`: The lower tick of the position
    /// * `tick_upper`: The upper tick of the position
    /// * `amount0`: token0 amount
    /// * `amount1`: token1 amount
    /// * `use_full_precision`: If false, liquidity will be maximized according to what the router can calculate,
    /// not what core can theoretically support
    ///
    /// returns: The position with the maximum amount of liquidity received
    ///
    pub fn from_amounts(
        pool: Pool,
        tick_lower: i32,
        tick_upper: i32,
        amount0: U256,
        amount1: U256,
        use_full_precision: bool,
    ) -> Self {
        let sqrt_ratio_a_x96 = get_sqrt_ratio_at_tick(tick_lower).unwrap();
        let sqrt_ratio_b_x96 = get_sqrt_ratio_at_tick(tick_upper).unwrap();
        let liquidity = max_liquidity_for_amounts(
            pool.sqrt_ratio_x96,
            sqrt_ratio_a_x96,
            sqrt_ratio_b_x96,
            amount0,
            amount1,
            use_full_precision,
        );
        Self::new(pool, liquidity.to_u128().unwrap(), tick_lower, tick_upper)
    }

    /// Computes a position with the maximum amount of liquidity received for a given amount of token0,
    /// assuming an unlimited amount of token1
    ///
    /// # Arguments
    ///
    /// * `pool`: The pool for which the position is created
    /// * `tick_lower`: The lower tick
    /// * `tick_upper`: The upper tick
    /// * `amount0`: The desired amount of token0
    /// * `use_full_precision`: If true, liquidity will be maximized according to what the router can calculate,
    /// not what core can theoretically support
    ///
    /// returns: Position
    ///
    pub fn from_amount0(
        pool: Pool,
        tick_lower: i32,
        tick_upper: i32,
        amount0: U256,
        use_full_precision: bool,
    ) -> Self {
        Self::from_amounts(
            pool,
            tick_lower,
            tick_upper,
            amount0,
            U256::MAX,
            use_full_precision,
        )
    }

    /// Computes a position with the maximum amount of liquidity received for a given amount of token1,
    /// assuming an unlimited amount of token0
    ///
    /// # Arguments
    ///
    /// * `pool`: The pool for which the position is created
    /// * `tick_lower`: The lower tick
    /// * `tick_upper`: The upper tick
    /// * `amount1`: The desired amount of token1
    ///
    /// returns: Position
    ///
    pub fn from_amount1(pool: Pool, tick_lower: i32, tick_upper: i32, amount1: U256) -> Self {
        // this function always uses full precision
        Self::from_amounts(pool, tick_lower, tick_upper, U256::MAX, amount1, true)
    }
}
