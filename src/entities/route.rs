use crate::prelude::Pool;
use alloy_primitives::ChainId;
use anyhow::Result;
use uniswap_sdk_core::prelude::*;

/// Represents a list of pools through which a swap can occur
#[derive(Clone, PartialEq, Debug)]
pub struct Route<TInput: CurrencyTrait, TOutput: CurrencyTrait, P> {
    pub pools: Vec<Pool<P>>,
    pub token_path: Vec<Token>,
    /// The input token
    pub input: TInput,
    /// The output token
    pub output: TOutput,
    _mid_price: Option<Price<TInput, TOutput>>,
}

impl<TInput: CurrencyTrait, TOutput: CurrencyTrait, P> Route<TInput, TOutput, P> {
    /// Creates an instance of route.
    ///
    /// ## Arguments
    ///
    /// * `pools`: An array of [`Pool`] objects, ordered by the route the swap will take
    /// * `input`: The input token
    /// * `output`: The output token
    pub fn new(pools: Vec<Pool<P>>, input: TInput, output: TOutput) -> Self {
        assert!(!pools.is_empty(), "POOLS");

        let chain_id = pools[0].chain_id();
        let all_on_same_chain = pools.iter().all(|pool| pool.chain_id() == chain_id);
        assert!(all_on_same_chain, "CHAIN_IDS");

        let wrapped_input = input.wrapped();
        assert!(pools[0].involves_token(&wrapped_input), "INPUT");

        assert!(
            pools.last().unwrap().involves_token(&output.wrapped()),
            "OUTPUT"
        );

        let mut token_path: Vec<Token> = Vec::with_capacity(pools.len() + 1);
        token_path.push(wrapped_input);
        for (i, pool) in pools.iter().enumerate() {
            let current_input_token = &token_path[i];
            assert!(
                current_input_token.equals(&pool.token0)
                    || current_input_token.equals(&pool.token1),
                "PATH"
            );
            let next_token = if current_input_token.equals(&pool.token0) {
                pool.token1.clone()
            } else {
                pool.token0.clone()
            };
            token_path.push(next_token);
        }
        assert!(token_path.last().unwrap().equals(&output.wrapped()), "PATH");

        Route {
            pools,
            token_path,
            input,
            output,
            _mid_price: None,
        }
    }

    pub fn chain_id(&self) -> ChainId {
        self.pools[0].chain_id()
    }

    /// Returns the mid price of the route
    pub fn mid_price(&mut self) -> Result<Price<TInput, TOutput>> {
        if let Some(mid_price) = &self._mid_price {
            return Ok(mid_price.clone());
        }
        let mut price: Price<Token, Token>;
        let mut next_input: Token;
        if self.pools[0].token0.equals(&self.input.wrapped()) {
            price = self.pools[0].token0_price();
            next_input = self.pools[0].token1.clone();
        } else {
            price = self.pools[0].token1_price();
            next_input = self.pools[0].token0.clone();
        }
        for pool in self.pools[1..].iter() {
            if next_input.equals(&pool.token0) {
                next_input = pool.token1.clone();
                price = price.multiply(&pool.token0_price())?;
            } else {
                next_input = pool.token0.clone();
                price = price.multiply(&pool.token1_price())?;
            }
        }
        self._mid_price = Some(Price::new(
            self.input.clone(),
            self.output.clone(),
            price.denominator(),
            price.numerator(),
        ));
        Ok(self._mid_price.clone().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prelude::*, tests::*};
    use once_cell::sync::Lazy;

    mod path {
        use super::*;

        #[test]
        fn constructs_a_path_from_the_tokens() {
            let route = Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone());
            assert_eq!(route.pools, vec![POOL_0_1.clone()]);
            assert_eq!(route.token_path, vec![TOKEN0.clone(), TOKEN1.clone()]);
            assert_eq!(route.input, TOKEN0.clone());
            assert_eq!(route.output, TOKEN1.clone());
            assert_eq!(route.chain_id(), 1);
        }

        #[test]
        #[should_panic]
        fn fails_if_the_input_is_not_in_the_first_pool() {
            Route::new(vec![POOL_0_1.clone()], WETH.clone(), TOKEN1.clone());
        }

        #[test]
        #[should_panic]
        fn fails_if_output_is_not_in_the_last_pool() {
            Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), WETH.clone());
        }

        #[test]
        fn can_have_a_token_as_both_input_and_output() {
            let route = Route::new(
                vec![POOL_0_WETH.clone(), POOL_0_1.clone(), POOL_1_WETH.clone()],
                WETH.clone(),
                WETH.clone(),
            );
            assert_eq!(
                route.pools,
                vec![POOL_0_WETH.clone(), POOL_0_1.clone(), POOL_1_WETH.clone()]
            );
            assert_eq!(route.input, WETH.clone());
            assert_eq!(route.output, WETH.clone());
        }

        #[test]
        fn supports_ether_input() {
            let route = Route::new(vec![POOL_0_WETH.clone()], ETHER.clone(), TOKEN0.clone());
            assert_eq!(route.pools, vec![POOL_0_WETH.clone()]);
            assert_eq!(route.input, ETHER.clone());
            assert_eq!(route.output, TOKEN0.clone());
        }

        #[test]
        fn supports_ether_output() {
            let route = Route::new(vec![POOL_0_WETH.clone()], TOKEN0.clone(), ETHER.clone());
            assert_eq!(route.pools, vec![POOL_0_WETH.clone()]);
            assert_eq!(route.input, TOKEN0.clone());
            assert_eq!(route.output, ETHER.clone());
        }
    }

    mod mid_price {
        use super::*;

        static POOL_0_1: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
            Pool::new(
                TOKEN0.clone(),
                TOKEN1.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(1, 5),
                0,
            )
            .unwrap()
        });
        static POOL_1_2: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
            Pool::new(
                TOKEN1.clone(),
                TOKEN2.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(15, 30),
                0,
            )
            .unwrap()
        });
        static POOL_0_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
            Pool::new(
                TOKEN0.clone(),
                WETH.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(3, 1),
                0,
            )
            .unwrap()
        });
        static POOL_1_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
            Pool::new(
                TOKEN1.clone(),
                WETH.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(1, 7),
                0,
            )
            .unwrap()
        });

        #[test]
        fn correct_for_0_1() {
            let mut route = Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone());
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "0.2000");
            assert_eq!(price.base_currency, TOKEN0.clone());
            assert_eq!(price.quote_currency, TOKEN1.clone());
        }

        #[test]
        fn is_cached() {
            let mut route = Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone());
            let price = route.mid_price().unwrap();
            assert_eq!(price, route.mid_price().unwrap());
        }

        #[test]
        fn correct_for_1_0() {
            let mut route = Route::new(vec![POOL_0_1.clone()], TOKEN1.clone(), TOKEN0.clone());
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "5.0000");
            assert_eq!(price.base_currency, TOKEN1.clone());
            assert_eq!(price.quote_currency, TOKEN0.clone());
        }

        #[test]
        fn correct_for_0_1_2() {
            let mut route = Route::new(
                vec![POOL_0_1.clone(), POOL_1_2.clone()],
                TOKEN0.clone(),
                TOKEN2.clone(),
            );
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "0.1000");
            assert_eq!(price.base_currency, TOKEN0.clone());
            assert_eq!(price.quote_currency, TOKEN2.clone());
        }

        #[test]
        fn correct_for_2_1_0() {
            let mut route = Route::new(
                vec![POOL_1_2.clone(), POOL_0_1.clone()],
                TOKEN2.clone(),
                TOKEN0.clone(),
            );
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "10.0000");
            assert_eq!(price.base_currency, TOKEN2.clone());
            assert_eq!(price.quote_currency, TOKEN0.clone());
        }

        #[test]
        fn correct_for_ether_0() {
            let mut route = Route::new(vec![POOL_0_WETH.clone()], ETHER.clone(), TOKEN0.clone());
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "0.3333");
            assert_eq!(price.base_currency, ETHER.clone());
            assert_eq!(price.quote_currency, TOKEN0.clone());
        }

        #[test]
        fn correct_for_1_weth() {
            let mut route = Route::new(vec![POOL_1_WETH.clone()], TOKEN1.clone(), WETH.clone());
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, Rounding::RoundHalfUp), "0.1429");
            assert_eq!(price.base_currency, TOKEN1.clone());
            assert_eq!(price.quote_currency, WETH.clone());
        }

        #[test]
        fn correct_for_ether_0_1_weth() {
            let mut route = Route::new(
                vec![POOL_0_WETH.clone(), POOL_0_1.clone(), POOL_1_WETH.clone()],
                ETHER.clone(),
                WETH.clone(),
            );
            let price = route.mid_price().unwrap();
            assert_eq!(
                price.to_significant(4, Rounding::RoundHalfUp).unwrap(),
                "0.009524"
            );
            assert_eq!(price.base_currency, ETHER.clone());
            assert_eq!(price.quote_currency, WETH.clone());
        }

        #[test]
        fn correct_for_weth_0_1_ether() {
            let mut route = Route::new(
                vec![POOL_0_WETH.clone(), POOL_0_1.clone(), POOL_1_WETH.clone()],
                WETH.clone(),
                ETHER.clone(),
            );
            let price = route.mid_price().unwrap();
            assert_eq!(
                price.to_significant(4, Rounding::RoundHalfUp).unwrap(),
                "0.009524"
            );
            assert_eq!(price.base_currency, WETH.clone());
            assert_eq!(price.quote_currency, ETHER.clone());
        }
    }
}
