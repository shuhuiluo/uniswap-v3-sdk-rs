use crate::prelude::*;
use alloy_primitives::{ChainId, B256, I256, U256};
use anyhow::Result;
use once_cell::sync::Lazy;
use std::{fmt, ops::Neg};
use uniswap_sdk_core::prelude::*;

static _Q192: Lazy<BigUint> = Lazy::new(|| u256_to_big_uint(Q192));

/// Represents a V3 pool
#[derive(Clone)]
pub struct Pool<P> {
    pub token0: Token,
    pub token1: Token,
    pub fee: FeeAmount,
    pub sqrt_ratio_x96: U256,
    pub liquidity: u128,
    pub tick_current: i32,
    pub tick_data_provider: P,
}

impl<P> fmt::Debug for Pool<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pool")
            .field("token0", &self.token0)
            .field("token1", &self.token1)
            .field("fee", &self.fee)
            .field("sqrt_ratio_x96", &self.sqrt_ratio_x96)
            .field("liquidity", &self.liquidity)
            .field("tick_current", &self.tick_current)
            .finish()
    }
}

impl<P> PartialEq for Pool<P> {
    fn eq(&self, other: &Self) -> bool {
        self.token0 == other.token0
            && self.token1 == other.token1
            && self.fee == other.fee
            && self.sqrt_ratio_x96 == other.sqrt_ratio_x96
            && self.liquidity == other.liquidity
            && self.tick_current == other.tick_current
    }
}

struct SwapState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x96: U256,
    tick: i32,
    liquidity: u128,
}

struct StepComputations {
    sqrt_price_start_x96: U256,
    tick_next: i32,
    initialized: bool,
    sqrt_price_next_x96: U256,
    amount_in: U256,
    amount_out: U256,
    fee_amount: U256,
}

impl Pool<NoTickDataProvider> {
    /// Construct a pool
    ///
    /// ## Arguments
    ///
    /// * `token_a`: One of the tokens in the pool
    /// * `token_b`: The other token in the pool
    /// * `fee`: The fee in hundredths of a bips of the input amount of every swap that is collected
    ///   by the pool
    /// * `sqrt_ratio_x96`: The sqrt of the current ratio of amounts of token1 to token0
    /// * `liquidity`: The current value of in range liquidity
    /// * `tick_current`: The current tick of the pool
    pub fn new(
        token_a: Token,
        token_b: Token,
        fee: FeeAmount,
        sqrt_ratio_x96: U256,
        liquidity: u128,
    ) -> Result<Pool<NoTickDataProvider>> {
        Self::new_with_tick_data_provider(
            token_a,
            token_b,
            fee,
            sqrt_ratio_x96,
            liquidity,
            NoTickDataProvider,
        )
    }
}

/// Compute the pool address
pub fn get_address(
    token_a: &Token,
    token_b: &Token,
    fee: FeeAmount,
    init_code_hash_manual_override: Option<B256>,
    factory_address_override: Option<Address>,
) -> Address {
    compute_pool_address(
        factory_address_override.unwrap_or(FACTORY_ADDRESS),
        token_a.address(),
        token_b.address(),
        fee,
        init_code_hash_manual_override,
    )
}

impl<P> Pool<P> {
    /// Returns the pool address
    pub fn address(
        &self,
        init_code_hash_manual_override: Option<B256>,
        factory_address_override: Option<Address>,
    ) -> Address {
        get_address(
            &self.token0,
            &self.token1,
            self.fee,
            init_code_hash_manual_override,
            factory_address_override,
        )
    }

    pub fn chain_id(&self) -> ChainId {
        self.token0.chain_id()
    }

    pub const fn tick_spacing(&self) -> i32 {
        self.fee.tick_spacing()
    }

    /// Returns true if the token is either token0 or token1
    ///
    /// ## Arguments
    ///
    /// * `token`: The token to check
    ///
    /// returns: bool
    pub fn involves_token(&self, token: &Token) -> bool {
        self.token0.equals(token) || self.token1.equals(token)
    }

    /// Returns the current mid price of the pool in terms of token0, i.e. the ratio of token1 over
    /// token0
    pub fn token0_price(&self) -> Price<Token, Token> {
        let sqrt_ratio_x96: BigUint = u256_to_big_uint(self.sqrt_ratio_x96);
        Price::new(
            self.token0.clone(),
            self.token1.clone(),
            _Q192.clone(),
            &sqrt_ratio_x96 * &sqrt_ratio_x96,
        )
    }

    /// Returns the current mid price of the pool in terms of token1, i.e. the ratio of token0 over
    /// token1
    pub fn token1_price(&self) -> Price<Token, Token> {
        let sqrt_ratio_x96: BigUint = u256_to_big_uint(self.sqrt_ratio_x96);
        Price::new(
            self.token1.clone(),
            self.token0.clone(),
            &sqrt_ratio_x96 * &sqrt_ratio_x96,
            _Q192.clone(),
        )
    }

    /// Return the price of the given token in terms of the other token in the pool.
    ///
    /// ## Arguments
    ///
    /// * `token`: The token to return price of
    ///
    /// returns: Price<Token, Token>
    pub fn price_of(&self, token: &Token) -> Price<Token, Token> {
        assert!(self.involves_token(token), "TOKEN");
        if self.token0.equals(token) {
            self.token0_price()
        } else {
            self.token1_price()
        }
    }
}

impl<T, P> Pool<P>
where
    T: TickTrait,
    P: TickDataProvider<Tick = T>,
{
    /// Construct a pool with a tick data provider
    ///
    /// ## Arguments
    ///
    /// * `token_a`: One of the tokens in the pool
    /// * `token_b`: The other token in the pool
    /// * `fee`: The fee in hundredths of a bips of the input amount of every swap that is collected
    ///   by the pool
    /// * `sqrt_ratio_x96`: The sqrt of the current ratio of amounts of token1 to token0
    /// * `liquidity`: The current value of in range liquidity
    /// * `tick_current`: The current tick of the pool
    /// * `tick_data_provider`: A tick data provider that can return tick data
    pub fn new_with_tick_data_provider(
        token_a: Token,
        token_b: Token,
        fee: FeeAmount,
        sqrt_ratio_x96: U256,
        liquidity: u128,
        tick_data_provider: P,
    ) -> Result<Self> {
        let (token0, token1) = if token_a.sorts_before(&token_b)? {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        Ok(Self {
            token0,
            token1,
            fee,
            sqrt_ratio_x96,
            liquidity,
            tick_current: get_tick_at_sqrt_ratio(sqrt_ratio_x96)?,
            tick_data_provider,
        })
    }

    /// Given an input amount of a token, return the computed output amount, and a pool with state
    /// updated after the trade
    ///
    /// ## Arguments
    ///
    /// * `input_amount`: The input amount for which to quote the output amount
    /// * `sqrt_price_limit_x96`: The Q64.96 sqrt price limit
    ///
    /// returns: The output amount and the pool with updated state
    pub fn get_output_amount(
        &self,
        input_amount: &CurrencyAmount<Token>,
        sqrt_price_limit_x96: Option<U256>,
    ) -> Result<(CurrencyAmount<Token>, Self)> {
        assert!(self.involves_token(&input_amount.currency), "TOKEN");

        let zero_for_one = input_amount.currency.equals(&self.token0);

        let (output_amount, sqrt_ratio_x96, liquidity, _) = self._swap(
            zero_for_one,
            big_int_to_i256(input_amount.quotient()),
            sqrt_price_limit_x96,
        )?;
        let output_token = if zero_for_one {
            self.token1.clone()
        } else {
            self.token0.clone()
        };
        Ok((
            CurrencyAmount::from_raw_amount(output_token, i256_to_big_int(output_amount.neg()))?,
            Pool::new_with_tick_data_provider(
                self.token0.clone(),
                self.token1.clone(),
                self.fee,
                sqrt_ratio_x96,
                liquidity,
                self.tick_data_provider.clone(),
            )?,
        ))
    }

    /// Given a desired output amount of a token, return the computed input amount and a pool with
    /// state updated after the trade
    ///
    /// ## Arguments
    ///
    /// * `output_amount`: the output amount for which to quote the input amount
    /// * `sqrt_price_limit_x96`: The Q64.96 sqrt price limit. If zero for one, the price cannot be
    ///   less than this value after the swap. If one for zero, the price cannot be greater than
    ///   this value after the swap
    ///
    /// returns: The input amount and the pool with updated state
    pub fn get_input_amount(
        &self,
        output_amount: &CurrencyAmount<Token>,
        sqrt_price_limit_x96: Option<U256>,
    ) -> Result<(CurrencyAmount<Token>, Self)> {
        assert!(self.involves_token(&output_amount.currency), "TOKEN");

        let zero_for_one = output_amount.currency.equals(&self.token1);

        let (input_amount, sqrt_ratio_x96, liquidity, _) = self._swap(
            zero_for_one,
            big_int_to_i256(output_amount.quotient()).neg(),
            sqrt_price_limit_x96,
        )?;
        let input_token = if zero_for_one {
            self.token0.clone()
        } else {
            self.token1.clone()
        };
        Ok((
            CurrencyAmount::from_raw_amount(input_token, i256_to_big_int(input_amount))?,
            Pool::new_with_tick_data_provider(
                self.token0.clone(),
                self.token1.clone(),
                self.fee,
                sqrt_ratio_x96,
                liquidity,
                self.tick_data_provider.clone(),
            )?,
        ))
    }

    fn _swap(
        &self,
        zero_for_one: bool,
        amount_specified: I256,
        sqrt_price_limit_x96: Option<U256>,
    ) -> Result<(I256, U256, u128, i32)> {
        let sqrt_price_limit_x96 = sqrt_price_limit_x96.unwrap_or_else(|| {
            if zero_for_one {
                MIN_SQRT_RATIO + ONE
            } else {
                MAX_SQRT_RATIO - ONE
            }
        });

        if zero_for_one {
            assert!(sqrt_price_limit_x96 > MIN_SQRT_RATIO, "RATIO_MIN");
            assert!(sqrt_price_limit_x96 < self.sqrt_ratio_x96, "RATIO_CURRENT");
        } else {
            assert!(sqrt_price_limit_x96 < MAX_SQRT_RATIO, "RATIO_MAX");
            assert!(sqrt_price_limit_x96 > self.sqrt_ratio_x96, "RATIO_CURRENT");
        }

        let exact_input = amount_specified >= I256::ZERO;

        // keep track of swap state
        let mut state = SwapState {
            amount_specified_remaining: amount_specified,
            amount_calculated: I256::ZERO,
            sqrt_price_x96: self.sqrt_ratio_x96,
            tick: self.tick_current,
            liquidity: self.liquidity,
        };

        // start swap while loop
        while !state.amount_specified_remaining.is_zero()
            && state.sqrt_price_x96 != sqrt_price_limit_x96
        {
            let mut step = StepComputations {
                sqrt_price_start_x96: state.sqrt_price_x96,
                tick_next: 0,
                initialized: false,
                sqrt_price_next_x96: U256::ZERO,
                amount_in: U256::ZERO,
                amount_out: U256::ZERO,
                fee_amount: U256::ZERO,
            };

            step.sqrt_price_start_x96 = state.sqrt_price_x96;
            // because each iteration of the while loop rounds, we can't optimize this code
            // (relative to the smart contract) by simply traversing to the next available tick, we
            // instead need to exactly replicate
            (step.tick_next, step.initialized) = self
                .tick_data_provider
                .next_initialized_tick_within_one_word(
                    state.tick,
                    zero_for_one,
                    self.tick_spacing(),
                )?;

            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            step.sqrt_price_next_x96 = get_sqrt_ratio_at_tick(step.tick_next)?;
            (
                state.sqrt_price_x96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = compute_swap_step(
                state.sqrt_price_x96,
                if zero_for_one {
                    step.sqrt_price_next_x96.max(sqrt_price_limit_x96)
                } else {
                    step.sqrt_price_next_x96.min(sqrt_price_limit_x96)
                },
                state.liquidity,
                state.amount_specified_remaining,
                self.fee as u32,
            )?;

            if exact_input {
                state.amount_specified_remaining = I256::from_raw(
                    state.amount_specified_remaining.into_raw() - step.amount_in - step.fee_amount,
                );
                state.amount_calculated =
                    I256::from_raw(state.amount_calculated.into_raw() - step.amount_out);
            } else {
                state.amount_specified_remaining =
                    I256::from_raw(state.amount_specified_remaining.into_raw() + step.amount_out);
                state.amount_calculated = I256::from_raw(
                    state.amount_calculated.into_raw() + step.amount_in + step.fee_amount,
                );
            }

            if state.sqrt_price_x96 == step.sqrt_price_next_x96 {
                // if the tick is initialized, run the tick transition
                if step.initialized {
                    let mut liquidity_net = self
                        .tick_data_provider
                        .get_tick(step.tick_next)?
                        .liquidity_net();
                    // if we're moving leftward, we interpret liquidityNet as the opposite sign
                    // safe because liquidityNet cannot be type(int128).min
                    if zero_for_one {
                        liquidity_net = liquidity_net.neg();
                    }
                    state.liquidity = add_delta(state.liquidity, liquidity_net)?;
                }
                state.tick = step.tick_next - zero_for_one as i32;
            } else {
                // recompute unless we're on a lower tick boundary (i.e. already transitioned
                // ticks), and haven't moved
                state.tick = get_tick_at_sqrt_ratio(state.sqrt_price_x96)?;
            }
        }

        Ok((
            state.amount_calculated,
            state.sqrt_price_x96,
            state.liquidity,
            state.tick,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;

    const ONE_ETHER: U256 = U256::from_limbs([10u64.pow(18), 0, 0, 0]);

    mod constructor {
        use super::*;

        #[test]
        #[should_panic(expected = "CHAIN_IDS")]
        fn cannot_be_used_for_tokens_on_different_chains() {
            let weth9 = WETH9::default().get(3).unwrap().clone();
            Pool::new(USDC.clone(), weth9.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0)
                .expect("CHAIN_IDS");
        }

        #[test]
        #[should_panic(expected = "ADDRESSES")]
        fn cannot_be_given_two_of_the_same_token() {
            Pool::new(USDC.clone(), USDC.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0)
                .expect("ADDRESSES");
        }

        #[test]
        fn works_with_valid_arguments_for_empty_pool_medium_fee() {
            let weth9 = WETH9::default().get(1).unwrap().clone();
            Pool::new(USDC.clone(), weth9.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0).unwrap();
        }

        #[test]
        fn works_with_valid_arguments_for_empty_pool_low_fee() {
            let weth9 = WETH9::default().get(1).unwrap().clone();
            Pool::new(USDC.clone(), weth9.clone(), FeeAmount::LOW, ONE_ETHER, 0).unwrap();
        }

        #[test]
        fn works_with_valid_arguments_for_empty_pool_lowest_fee() {
            let weth9 = WETH9::default().get(1).unwrap().clone();
            Pool::new(USDC.clone(), weth9.clone(), FeeAmount::LOWEST, ONE_ETHER, 0).unwrap();
        }

        #[test]
        fn works_with_valid_arguments_for_empty_pool_high_fee() {
            let weth9 = WETH9::default().get(1).unwrap().clone();
            Pool::new(USDC.clone(), weth9.clone(), FeeAmount::HIGH, ONE_ETHER, 0).unwrap();
        }
    }

    #[test]
    fn get_address_matches_an_example() {
        let result = get_address(&USDC, &DAI, FeeAmount::LOW, None, None);
        assert_eq!(result, address!("6c6Bc977E13Df9b0de53b251522280BB72383700"));
    }

    #[test]
    fn token0_always_is_the_token_that_sorts_before() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert!(pool.token0.equals(&DAI.clone()));
        let pool = Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert!(pool.token0.equals(&DAI.clone()));
    }

    #[test]
    fn token1_always_is_the_token_that_sorts_after() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert!(pool.token1.equals(&USDC.clone()));
        let pool = Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert!(pool.token1.equals(&USDC.clone()));
    }

    #[test]
    fn token0_price_returns_price_of_token0_in_terms_of_token1() -> Result<()> {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(101e6 as u128, 100e18 as u128),
            0,
        )?;
        assert_eq!(
            pool.token0_price()
                .to_significant(5, Rounding::RoundHalfUp)?,
            "1.01"
        );
        let pool = Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(101e6 as u128, 100e18 as u128),
            0,
        )?;
        assert_eq!(
            pool.token0_price()
                .to_significant(5, Rounding::RoundHalfUp)?,
            "1.01"
        );
        Ok(())
    }

    #[test]
    fn token1_price_returns_price_of_token1_in_terms_of_token0() -> Result<()> {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(101e6 as u128, 100e18 as u128),
            0,
        )?;
        assert_eq!(
            pool.token1_price()
                .to_significant(5, Rounding::RoundHalfUp)?,
            "0.9901"
        );
        let pool = Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(101e6 as u128, 100e18 as u128),
            0,
        )?;
        assert_eq!(
            pool.token1_price()
                .to_significant(5, Rounding::RoundHalfUp)?,
            "0.9901"
        );
        Ok(())
    }

    #[test]
    fn price_of_returns_price_of_token_in_terms_of_other_token() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert_eq!(pool.price_of(&DAI.clone()), pool.token0_price());
        assert_eq!(pool.price_of(&USDC.clone()), pool.token1_price());
    }

    #[test]
    #[should_panic(expected = "TOKEN")]
    fn price_of_throws_if_invalid_token() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        pool.price_of(&WETH9::default().get(1).unwrap().clone());
    }

    #[test]
    fn chain_id_returns_token0_chain_id() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert_eq!(pool.chain_id(), 1);
        let pool = Pool::new(
            DAI.clone(),
            USDC.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert_eq!(pool.chain_id(), 1);
    }

    #[test]
    fn involves_token() {
        let pool = Pool::new(
            USDC.clone(),
            DAI.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap();
        assert!(pool.involves_token(&USDC.clone()));
        assert!(pool.involves_token(&DAI.clone()));
        assert!(!pool.involves_token(&WETH9::default().get(1).unwrap().clone()));
    }

    mod swaps {
        use super::*;

        static POOL: Lazy<Pool<TickListDataProvider>> = Lazy::new(|| {
            Pool::new_with_tick_data_provider(
                USDC.clone(),
                DAI.clone(),
                FeeAmount::LOW,
                encode_sqrt_ratio_x96(1, 1),
                ONE_ETHER.into_limbs()[0] as u128,
                TickListDataProvider::new(
                    vec![
                        Tick::new(
                            nearest_usable_tick(MIN_TICK, FeeAmount::LOW.tick_spacing()),
                            ONE_ETHER.into_limbs()[0] as u128,
                            ONE_ETHER.into_limbs()[0] as i128,
                        ),
                        Tick::new(
                            nearest_usable_tick(MAX_TICK, FeeAmount::LOW.tick_spacing()),
                            ONE_ETHER.into_limbs()[0] as u128,
                            -(ONE_ETHER.into_limbs()[0] as i128),
                        ),
                    ],
                    FeeAmount::LOW.tick_spacing(),
                ),
            )
            .unwrap()
        });

        #[test]
        fn get_output_amount_usdc_to_dai() -> Result<()> {
            let (output_amount, _) =
                POOL.get_output_amount(&CurrencyAmount::from_raw_amount(USDC.clone(), 100)?, None)?;
            assert!(output_amount.currency.equals(&DAI.clone()));
            assert_eq!(output_amount.quotient(), 98.into());
            Ok(())
        }

        #[test]
        fn get_output_amount_dai_to_usdc() -> Result<()> {
            let (output_amount, _) =
                POOL.get_output_amount(&CurrencyAmount::from_raw_amount(DAI.clone(), 100)?, None)?;
            assert!(output_amount.currency.equals(&USDC.clone()));
            assert_eq!(output_amount.quotient(), 98.into());
            Ok(())
        }

        #[test]
        fn get_input_amount_usdc_to_dai() -> Result<()> {
            let (input_amount, _) =
                POOL.get_input_amount(&CurrencyAmount::from_raw_amount(DAI.clone(), 98)?, None)?;
            assert!(input_amount.currency.equals(&USDC.clone()));
            assert_eq!(input_amount.quotient(), 100.into());
            Ok(())
        }

        #[test]
        fn get_input_amount_dai_to_usdc() -> Result<()> {
            let (input_amount, _) =
                POOL.get_input_amount(&CurrencyAmount::from_raw_amount(USDC.clone(), 98)?, None)?;
            assert!(input_amount.currency.equals(&DAI.clone()));
            assert_eq!(input_amount.quotient(), 100.into());
            Ok(())
        }
    }
}
