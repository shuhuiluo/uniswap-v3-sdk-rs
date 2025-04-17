use crate::prelude::{Error, *};
use alloy_primitives::ChainId;
use uniswap_sdk_core::prelude::*;

/// Represents a list of pools through which a swap can occur
#[derive(Clone, PartialEq, Debug)]
pub struct Route<TInput, TOutput, TP>
where
    TInput: BaseCurrency,
    TOutput: BaseCurrency,
    TP: TickDataProvider,
{
    pub pools: Vec<Pool<TP>>,
    /// The input token
    pub input: TInput,
    /// The output token
    pub output: TOutput,
    _mid_price: Option<Price<TInput, TOutput>>,
}

impl<TInput, TOutput, TP> Route<TInput, TOutput, TP>
where
    TInput: BaseCurrency,
    TOutput: BaseCurrency,
    TP: TickDataProvider,
{
    /// Creates an instance of route.
    ///
    /// ## Arguments
    ///
    /// * `pools`: An array of [`Pool`] objects, ordered by the route the swap will take
    /// * `input`: The input token
    /// * `output`: The output token
    #[inline]
    pub fn new(pools: Vec<Pool<TP>>, input: TInput, output: TOutput) -> Self {
        assert!(!pools.is_empty(), "POOLS");

        let chain_id = pools[0].chain_id();
        let all_on_same_chain = pools.iter().all(|pool| pool.chain_id() == chain_id);
        assert!(all_on_same_chain, "CHAIN_IDS");

        let wrapped_input = input.wrapped();
        assert!(pools[0].involves_token(wrapped_input), "INPUT");

        let wrapped_output = output.wrapped();
        assert!(
            pools.last().unwrap().involves_token(wrapped_output),
            "OUTPUT"
        );

        let mut current_input_token = wrapped_input;
        for pool in &pools {
            current_input_token = if current_input_token.equals(&pool.token0) {
                &pool.token1
            } else if current_input_token.equals(&pool.token1) {
                &pool.token0
            } else {
                panic!("PATH")
            };
        }
        assert!(current_input_token.equals(wrapped_output), "PATH");

        Self {
            pools,
            input,
            output,
            _mid_price: None,
        }
    }

    /// Returns the path of tokens that the route will take
    #[inline]
    pub fn token_path(&self) -> Vec<Token> {
        let mut token_path: Vec<Token> = Vec::with_capacity(self.pools.len() + 1);
        token_path.push(self.input.wrapped().clone());
        for (i, pool) in self.pools.iter().enumerate() {
            let next_token = if token_path[i].equals(&pool.token0) {
                pool.token1.clone()
            } else {
                pool.token0.clone()
            };
            token_path.push(next_token);
        }
        token_path
    }

    #[inline]
    pub fn chain_id(&self) -> ChainId {
        self.pools[0].chain_id()
    }

    /// Returns the mid price of the route
    #[inline]
    pub fn mid_price(&self) -> Result<Price<TInput, TOutput>, Error> {
        let mut price = self.pools[0].price_of(self.input.wrapped())?;
        for pool in &self.pools[1..] {
            price = price.multiply(&pool.price_of(&price.quote_currency)?)?;
        }
        Ok(Price::new(
            self.input.clone(),
            self.output.clone(),
            price.denominator,
            price.numerator,
        ))
    }

    /// Returns the cached mid price of the route
    #[inline]
    pub fn mid_price_cached(&mut self) -> Result<Price<TInput, TOutput>, Error> {
        if let Some(mid_price) = &self._mid_price {
            return Ok(mid_price.clone());
        }
        let mid_price = self.mid_price()?;
        self._mid_price = Some(mid_price.clone());
        Ok(mid_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_route, tests::*};
    use once_cell::sync::Lazy;

    mod path {
        use super::*;

        #[test]
        fn constructs_a_path_from_the_tokens() {
            let route = ROUTE_0_1.clone();
            assert_eq!(route.pools, vec![POOL_0_1.clone()]);
            assert_eq!(route.token_path(), vec![TOKEN0.clone(), TOKEN1.clone()]);
            assert_eq!(route.input, *TOKEN0);
            assert_eq!(route.output, *TOKEN1);
            assert_eq!(route.chain_id(), 1);
        }

        #[test]
        #[should_panic(expected = "INPUT")]
        fn fails_if_the_input_is_not_in_the_first_pool() {
            create_route!(POOL_0_1, WETH, TOKEN1);
        }

        #[test]
        #[should_panic(expected = "OUTPUT")]
        fn fails_if_output_is_not_in_the_last_pool() {
            create_route!(POOL_0_1, TOKEN0, WETH);
        }

        #[test]
        fn can_have_a_token_as_both_input_and_output() {
            let route = create_route!(POOL_0_WETH, POOL_0_1, POOL_1_WETH; WETH, WETH);
            assert_eq!(
                route.pools,
                vec![POOL_0_WETH.clone(), POOL_0_1.clone(), POOL_1_WETH.clone()]
            );
            assert_eq!(route.input, *WETH);
            assert_eq!(route.output, *WETH);
        }

        #[test]
        fn supports_ether_input() {
            let route = ROUTE_ETH_0.clone();
            assert_eq!(route.pools, vec![POOL_0_WETH.clone()]);
            assert_eq!(route.input, *ETHER);
            assert_eq!(route.output, *TOKEN0);
        }

        #[test]
        fn supports_ether_output() {
            let route = create_route!(POOL_0_WETH, TOKEN0, ETHER);
            assert_eq!(route.pools, vec![POOL_0_WETH.clone()]);
            assert_eq!(route.input, *TOKEN0);
            assert_eq!(route.output, *ETHER);
        }
    }

    mod mid_price {
        use super::*;

        static POOL_0_1: Lazy<Pool> = Lazy::new(|| {
            Pool::new(
                TOKEN0.clone(),
                TOKEN1.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(1, 5),
                0,
            )
            .unwrap()
        });
        static POOL_1_2: Lazy<Pool> = Lazy::new(|| {
            Pool::new(
                TOKEN1.clone(),
                TOKEN2.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(15, 30),
                0,
            )
            .unwrap()
        });
        static POOL_0_WETH: Lazy<Pool> = Lazy::new(|| {
            Pool::new(
                TOKEN0.clone(),
                WETH.clone(),
                FeeAmount::MEDIUM,
                encode_sqrt_ratio_x96(3, 1),
                0,
            )
            .unwrap()
        });
        static POOL_1_WETH: Lazy<Pool> = Lazy::new(|| {
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
            let route = create_route!(POOL_0_1; TOKEN0, TOKEN1);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "0.2000");
            assert_eq!(price.base_currency, *TOKEN0);
            assert_eq!(price.quote_currency, *TOKEN1);
        }

        #[test]
        fn is_cached() {
            let mut route = create_route!(POOL_0_1; TOKEN0, TOKEN1);
            let price = route.mid_price_cached().unwrap();
            assert_eq!(price, route._mid_price.unwrap());
        }

        #[test]
        fn correct_for_1_0() {
            let route = create_route!(POOL_0_1; TOKEN1, TOKEN0);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "5.0000");
            assert_eq!(price.base_currency, *TOKEN1);
            assert_eq!(price.quote_currency, *TOKEN0);
        }

        #[test]
        fn correct_for_0_1_2() {
            let route = create_route!(POOL_0_1, POOL_1_2; TOKEN0, TOKEN2);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "0.1000");
            assert_eq!(price.base_currency, *TOKEN0);
            assert_eq!(price.quote_currency, *TOKEN2);
        }

        #[test]
        fn correct_for_2_1_0() {
            let route = create_route!(POOL_1_2, POOL_0_1; TOKEN2, TOKEN0);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "10.0000");
            assert_eq!(price.base_currency, *TOKEN2);
            assert_eq!(price.quote_currency, *TOKEN0);
        }

        #[test]
        fn correct_for_ether_0() {
            let route = create_route!(POOL_0_WETH; ETHER, TOKEN0);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "0.3333");
            assert_eq!(price.base_currency, *ETHER);
            assert_eq!(price.quote_currency, *TOKEN0);
        }

        #[test]
        fn correct_for_1_weth() {
            let route = create_route!(POOL_1_WETH; TOKEN1, WETH);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_fixed(4, None), "0.1429");
            assert_eq!(price.base_currency, *TOKEN1);
            assert_eq!(price.quote_currency, *WETH);
        }

        #[test]
        fn correct_for_ether_0_1_weth() {
            let route = create_route!( POOL_0_WETH, POOL_0_1, POOL_1_WETH; ETHER, WETH);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_significant(4, None).unwrap(), "0.009524");
            assert_eq!(price.base_currency, *ETHER);
            assert_eq!(price.quote_currency, *WETH);
        }

        #[test]
        fn correct_for_weth_0_1_ether() {
            let route = create_route!(POOL_0_WETH, POOL_0_1, POOL_1_WETH; WETH, ETHER);
            let price = route.mid_price().unwrap();
            assert_eq!(price.to_significant(4, None).unwrap(), "0.009524");
            assert_eq!(price.base_currency, *WETH);
            assert_eq!(price.quote_currency, *ETHER);
        }
    }
}
