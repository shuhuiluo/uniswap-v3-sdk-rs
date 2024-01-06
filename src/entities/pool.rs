use crate::prelude::*;
use alloy_primitives::{Address, B256, U256};
use num_bigint::BigUint;
use once_cell::sync::Lazy;
use uniswap_sdk_core::prelude::*;

static _Q192: Lazy<BigUint> = Lazy::new(|| u256_to_big_uint(Q192));

/// Represents a V3 pool
#[derive(Clone)]
pub struct Pool {
    pub token0: Token,
    pub token1: Token,
    pub fee: FeeAmount,
    pub sqrt_ratio_x96: U256,
    pub liquidity: u128,
    pub tick_current: i32,
    _token0_price: Option<Price<Token, Token>>,
    _token1_price: Option<Price<Token, Token>>,
}

impl Pool {
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

    /// Construct a pool
    ///
    /// # Arguments
    ///
    /// * `token_a`: One of the tokens in the pool
    /// * `token_b`: The other token in the pool
    /// * `fee`: The fee in hundredths of a bips of the input amount of every swap that is collected by the pool
    /// * `sqrt_ratio_x96`: The sqrt of the current ratio of amounts of token1 to token0
    /// * `liquidity`: The current value of in range liquidity
    /// * `tick_current`: The current tick of the pool
    pub fn new(
        token_a: Token,
        token_b: Token,
        fee: FeeAmount,
        sqrt_ratio_x96: U256,
        liquidity: u128,
    ) -> Self {
        let (token0, token1) = if token_a.sorts_before(&token_b) {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        Self {
            token0,
            token1,
            fee,
            sqrt_ratio_x96,
            liquidity,
            tick_current: get_tick_at_sqrt_ratio(sqrt_ratio_x96).unwrap(),
            _token0_price: None,
            _token1_price: None,
        }
    }

    pub fn chain_id(&self) -> u32 {
        self.token0.chain_id()
    }

    pub const fn tick_spacing(&self) -> i32 {
        self.fee.tick_spacing()
    }

    /// Returns true if the token is either token0 or token1
    ///
    /// # Arguments
    ///
    /// * `token`: The token to check
    ///
    /// returns: bool
    ///
    pub fn involves_token(&self, token: &Token) -> bool {
        self.token0.equals(token) || self.token1.equals(token)
    }

    /// Returns the current mid price of the pool in terms of token0, i.e. the ratio of token1 over token0
    pub fn token0_price(&mut self) -> Price<Token, Token> {
        let sqrt_ratio_x96: BigUint = u256_to_big_uint(self.sqrt_ratio_x96);
        self._token0_price.clone().unwrap_or_else(|| {
            let price = Price::new(
                self.token0.clone(),
                self.token1.clone(),
                _Q192.clone(),
                &sqrt_ratio_x96 * &sqrt_ratio_x96,
            );
            self._token0_price = Some(price.clone());
            price
        })
    }

    /// Returns the current mid price of the pool in terms of token1, i.e. the ratio of token0 over token1
    pub fn token1_price(&mut self) -> Price<Token, Token> {
        let sqrt_ratio_x96: BigUint = u256_to_big_uint(self.sqrt_ratio_x96);
        self._token1_price.clone().unwrap_or_else(|| {
            let price = Price::new(
                self.token1.clone(),
                self.token0.clone(),
                &sqrt_ratio_x96 * &sqrt_ratio_x96,
                _Q192.clone(),
            );
            self._token1_price = Some(price.clone());
            price
        })
    }

    /// Return the price of the given token in terms of the other token in the pool.
    ///
    /// # Arguments
    ///
    /// * `token`: The token to return price of
    ///
    /// returns: Price<Token, Token>
    ///
    pub fn price_of(&mut self, token: &Token) -> Price<Token, Token> {
        assert!(self.involves_token(token), "TOKEN");
        if self.token0.equals(token) {
            self.token0_price()
        } else {
            self.token1_price()
        }
    }

    pub async fn get_output_amount(
        &self,
        _input_amount: CurrencyAmount<Token>,
        _sqrt_price_limit_x96: Option<U256>,
    ) -> (CurrencyAmount<Token>, Self) {
        todo!("get_output_amount")
    }

    pub async fn get_input_amount(
        &self,
        _output_amount: CurrencyAmount<Token>,
        _sqrt_price_limit_x96: Option<U256>,
    ) -> (CurrencyAmount<Token>, Self) {
        todo!("get_input_amount")
    }

    async fn _swap(
        &self,
        _zero_for_one: bool,
        _amount_specified: U256,
        _sqrt_price_limit_x96: Option<U256>,
    ) -> (U256, U256, u128, i32) {
        todo!("swap")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniswap_sdk_core::token;

    const ONE_ETHER: U256 = U256::from_limbs([10u64.pow(18), 0, 0, 0]);

    static USDC: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            6,
            "USDC",
            "USD Coin"
        )
    });
    static _DAI: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x6B175474E89094C44Da98b954EedeAC495271d0F",
            18,
            "DAI",
            "DAI Stablecoin"
        )
    });

    #[test]
    #[should_panic(expected = "CHAIN_IDS")]
    fn test_constructor_cannot_be_used_for_tokens_on_different_chains() {
        let weth9 = WETH9::default().get(3).unwrap().clone();
        Pool::new(USDC.clone(), weth9.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0);
    }

    #[test]
    #[should_panic(expected = "ADDRESSES")]
    fn test_constructor_cannot_be_given_two_of_the_same_token() {
        Pool::new(USDC.clone(), USDC.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0);
    }

    #[test]
    fn test_constructor_works_with_valid_arguments_for_empty_pool_medium_fee() {
        let weth9 = WETH9::default().get(1).unwrap().clone();
        Pool::new(USDC.clone(), weth9.clone(), FeeAmount::MEDIUM, ONE_ETHER, 0);
    }

    #[test]
    fn test_constructor_works_with_valid_arguments_for_empty_pool_low_fee() {
        let weth9 = WETH9::default().get(1).unwrap().clone();
        Pool::new(USDC.clone(), weth9.clone(), FeeAmount::LOW, ONE_ETHER, 0);
    }

    #[test]
    fn test_constructor_works_with_valid_arguments_for_empty_pool_lowest_fee() {
        let weth9 = WETH9::default().get(1).unwrap().clone();
        Pool::new(USDC.clone(), weth9.clone(), FeeAmount::LOWEST, ONE_ETHER, 0);
    }

    #[test]
    fn test_constructor_works_with_valid_arguments_for_empty_pool_high_fee() {
        let weth9 = WETH9::default().get(1).unwrap().clone();
        Pool::new(USDC.clone(), weth9.clone(), FeeAmount::HIGH, ONE_ETHER, 0);
    }
}
