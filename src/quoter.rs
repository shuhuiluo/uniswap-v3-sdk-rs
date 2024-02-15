use crate::prelude::*;
use alloy_primitives::U256;
use alloy_sol_types::SolCall;
use uniswap_sdk_core::{constants::TradeType, prelude::*};

/// Optional arguments to send to the quoter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuoteOptions {
    /// The price limit for the trade.
    sqrt_price_limit_x96: U256,
    /// The quoter interface to use
    use_quoter_v2: bool,
}

/// Produces the on-chain method name of the appropriate function within QuoterV2,
/// and the relevant hex encoded parameters.
///
/// ## Arguments
///
/// * `route`: The swap route, a list of pools through which a swap can occur
/// * `amount`: The amount of the quote, either an amount in, or an amount out
/// * `trade_type`: The trade type, either exact input or exact output
/// * `options`: The optional params including price limit and Quoter contract switch
///
pub fn quote_call_parameters<TInput: CurrencyTrait, TOutput: CurrencyTrait, P>(
    route: &Route<TInput, TOutput, P>,
    amount: CurrencyAmount<impl CurrencyTrait>,
    trade_type: TradeType,
    options: Option<QuoteOptions>,
) -> MethodParameters {
    let options = options.unwrap_or_default();
    let quote_amount = big_int_to_u256(amount.quotient());

    let calldata = if route.pools.len() == 1 {
        match trade_type {
            TradeType::ExactInput => {
                if options.use_quoter_v2 {
                    IQuoterV2::quoteExactInputSingleCall {
                        params: IQuoterV2::QuoteExactInputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            amountIn: quote_amount,
                            fee: route.pools[0].fee as u32,
                            sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                        },
                    }
                    .abi_encode()
                } else {
                    IQuoter::quoteExactInputSingleCall {
                        tokenIn: route.token_path[0].address(),
                        tokenOut: route.token_path[1].address(),
                        amountIn: quote_amount,
                        fee: route.pools[0].fee as u32,
                        sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                    }
                    .abi_encode()
                }
            }
            TradeType::ExactOutput => {
                if options.use_quoter_v2 {
                    IQuoterV2::quoteExactOutputSingleCall {
                        params: IQuoterV2::QuoteExactOutputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            amount: quote_amount,
                            fee: route.pools[0].fee as u32,
                            sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                        },
                    }
                    .abi_encode()
                } else {
                    IQuoter::quoteExactOutputSingleCall {
                        tokenIn: route.token_path[0].address(),
                        tokenOut: route.token_path[1].address(),
                        amountOut: quote_amount,
                        fee: route.pools[0].fee as u32,
                        sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                    }
                    .abi_encode()
                }
            }
        }
    } else {
        assert!(
            options.sqrt_price_limit_x96.is_zero(),
            "MULTIHOP_PRICE_LIMIT"
        );
        let path = encode_route_to_path(route, trade_type == TradeType::ExactOutput);
        match trade_type {
            TradeType::ExactInput => IQuoter::quoteExactInputCall {
                path,
                amountIn: quote_amount,
            }
            .abi_encode(),
            TradeType::ExactOutput => IQuoter::quoteExactOutputCall {
                path,
                amountOut: quote_amount,
            }
            .abi_encode(),
        }
    };
    MethodParameters {
        calldata,
        value: U256::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use uniswap_sdk_core::token;

    static TOKEN0: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x0000000000000000000000000000000000000001",
            18,
            "t0",
            "token0"
        )
    });
    static TOKEN1: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x0000000000000000000000000000000000000002",
            18,
            "t1",
            "token1"
        )
    });
    static WETH: Lazy<Token> = Lazy::new(|| Ether::on_chain(1).wrapped());
    const FEE_AMOUNT: FeeAmount = FeeAmount::MEDIUM;
    const SQRT_RATIO_X96: U256 = Q96;
    const LIQUIDITY: u128 = 1_000_000;

    fn make_pool(token0: Token, token1: Token) -> Pool<TickListDataProvider> {
        Pool::new_with_tick_data_provider(
            token0,
            token1,
            FEE_AMOUNT,
            SQRT_RATIO_X96,
            LIQUIDITY,
            TickListDataProvider::new(
                vec![
                    Tick::new(
                        nearest_usable_tick(MIN_TICK, FEE_AMOUNT.tick_spacing()),
                        LIQUIDITY,
                        LIQUIDITY as i128,
                    ),
                    Tick::new(
                        nearest_usable_tick(MAX_TICK, FEE_AMOUNT.tick_spacing()),
                        LIQUIDITY,
                        -(LIQUIDITY as i128),
                    ),
                ],
                FEE_AMOUNT.tick_spacing(),
            ),
        )
        .unwrap()
    }

    static POOL_0_1: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN0.clone(), TOKEN1.clone()));
    static POOL_1_WETH: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN1.clone(), WETH.clone()));

    mod single_trade_input {
        use super::*;
        use alloy_primitives::hex;

        #[test]
        fn single_hop_exact_input() {
            let mut trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let input_amount = trade.input_amount().unwrap();
            let params =
                quote_call_parameters(&trade.swaps[0].route, input_amount, trade.trade_type, None);
            assert_eq!(
                params.calldata,
                hex!("f7729d43000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[test]
        fn single_hop_exact_output() {
            let mut trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let output_amount = trade.output_amount().unwrap();
            let params =
                quote_call_parameters(&trade.swaps[0].route, output_amount, trade.trade_type, None);
            assert_eq!(
                params.calldata,
                hex!("30d07f21000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[test]
        fn multi_hop_exact_input() {
            let mut trade = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_WETH.clone()],
                    TOKEN0.clone(),
                    WETH.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let params = quote_call_parameters(
                &trade.route(),
                trade.input_amount().unwrap(),
                trade.trade_type,
                None,
            );
            assert_eq!(
                params.calldata,
                hex!("cdca17530000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[test]
        fn multi_hop_exact_output() {
            let mut trade = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_WETH.clone()],
                    TOKEN0.clone(),
                    WETH.clone(),
                ),
                CurrencyAmount::from_raw_amount(WETH.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let params = quote_call_parameters(
                &trade.route(),
                trade.output_amount().unwrap(),
                trade.trade_type,
                None,
            );
            assert_eq!(
                params.calldata,
                hex!("2f80bb1d000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[test]
        fn sqrt_price_limit_x96() {
            let mut trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let params = quote_call_parameters(
                &trade.route(),
                trade.input_amount().unwrap(),
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U256::from_limbs([0, 0, 1, 0]),
                    use_quoter_v2: false,
                }),
            );
            assert_eq!(
                params.calldata,
                hex!("f7729d43000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000100000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }
    }

    mod single_trade_input_using_quoter_v2 {
        use super::*;
        use alloy_primitives::hex;

        #[test]
        fn single_hop_exact_input() {
            let mut trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let input_amount = trade.input_amount().unwrap();
            let params = quote_call_parameters(
                &trade.swaps[0].route,
                input_amount,
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U256::ZERO,
                    use_quoter_v2: true,
                }),
            );
            assert_eq!(
                params.calldata,
                hex!("c6a5026a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000"),
            );
        }

        #[test]
        fn single_hop_exact_output() {
            let mut trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let output_amount = trade.output_amount().unwrap();
            let params = quote_call_parameters(
                &trade.swaps[0].route,
                output_amount,
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U256::ZERO,
                    use_quoter_v2: true,
                }),
            );
            assert_eq!(
                params.calldata,
                hex!("bd21704a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000"),
            );
        }
    }
}
