use crate::prelude::*;
use alloy_primitives::{U160, U256};
use alloy_sol_types::SolCall;
use uniswap_sdk_core::prelude::*;

/// Optional arguments to send to the quoter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuoteOptions {
    /// The price limit for the trade.
    pub sqrt_price_limit_x96: U160,
    /// The quoter interface to use
    pub use_quoter_v2: bool,
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
#[inline]
pub fn quote_call_parameters<TInput, TOutput, TP>(
    route: &Route<TInput, TOutput, TP>,
    amount: &CurrencyAmount<impl BaseCurrency>,
    trade_type: TradeType,
    options: Option<QuoteOptions>,
) -> MethodParameters
where
    TInput: BaseCurrency,
    TOutput: BaseCurrency,
    TP: TickDataProvider,
{
    let options = options.unwrap_or_default();
    let quote_amount = U256::from_big_int(amount.quotient());

    let calldata = if route.pools.len() == 1 {
        match trade_type {
            TradeType::ExactInput => {
                if options.use_quoter_v2 {
                    IQuoterV2::quoteExactInputSingleCall {
                        params: IQuoterV2::QuoteExactInputSingleParams {
                            tokenIn: route.input.wrapped().address(),
                            tokenOut: route.output.wrapped().address(),
                            amountIn: quote_amount,
                            fee: route.pools[0].fee.into(),
                            sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                        },
                    }
                    .abi_encode()
                } else {
                    IQuoter::quoteExactInputSingleCall {
                        tokenIn: route.input.wrapped().address(),
                        tokenOut: route.output.wrapped().address(),
                        amountIn: quote_amount,
                        fee: route.pools[0].fee.into(),
                        sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                    }
                    .abi_encode()
                }
            }
            TradeType::ExactOutput => {
                if options.use_quoter_v2 {
                    IQuoterV2::quoteExactOutputSingleCall {
                        params: IQuoterV2::QuoteExactOutputSingleParams {
                            tokenIn: route.input.wrapped().address(),
                            tokenOut: route.output.wrapped().address(),
                            amount: quote_amount,
                            fee: route.pools[0].fee.into(),
                            sqrtPriceLimitX96: options.sqrt_price_limit_x96,
                        },
                    }
                    .abi_encode()
                } else {
                    IQuoter::quoteExactOutputSingleCall {
                        tokenIn: route.input.wrapped().address(),
                        tokenOut: route.output.wrapped().address(),
                        amountOut: quote_amount,
                        fee: route.pools[0].fee.into(),
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
        calldata: calldata.into(),
        value: U256::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_route, tests::*, trade_from_route};
    use once_cell::sync::Lazy;

    static POOL_0_1: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN0.clone(), TOKEN1.clone()));
    static POOL_1_WETH: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN1.clone(), WETH.clone()));
    static ROUTE_0_1: Lazy<Route<Token, Token, TickListDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_1, TOKEN0, TOKEN1));
    static ROUTE_0_1_WETH: Lazy<Route<Token, Token, TickListDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_1, POOL_1_WETH; TOKEN0, WETH));

    mod single_trade_input {
        use super::*;
        use crate::currency_amount;
        use alloy_primitives::hex;

        #[tokio::test]
        async fn single_hop_exact_input() {
            let trade = trade_from_route!(ROUTE_0_1, TOKEN0_AMOUNT_100, TradeType::ExactInput);
            let input_amount = trade.input_amount().unwrap();
            let params =
                quote_call_parameters(&trade.swaps[0].route, &input_amount, trade.trade_type, None);
            assert_eq!(
                params.calldata.to_vec(),
                hex!("f7729d43000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[tokio::test]
        async fn single_hop_exact_output() {
            let trade = trade_from_route!(ROUTE_0_1, TOKEN1_AMOUNT_100, TradeType::ExactOutput);
            let output_amount = trade.output_amount().unwrap();
            let params = quote_call_parameters(
                &trade.swaps[0].route,
                &output_amount,
                trade.trade_type,
                None,
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("30d07f21000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[tokio::test]
        async fn multi_hop_exact_input() {
            let trade = trade_from_route!(ROUTE_0_1_WETH, TOKEN0_AMOUNT_100, TradeType::ExactInput);
            let params = quote_call_parameters(
                trade.route(),
                &trade.input_amount().unwrap(),
                trade.trade_type,
                None,
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("cdca17530000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[tokio::test]
        async fn multi_hop_exact_output() {
            let trade = trade_from_route!(
                ROUTE_0_1_WETH,
                currency_amount!(WETH, 100),
                TradeType::ExactOutput
            );
            let params = quote_call_parameters(
                trade.route(),
                &trade.output_amount().unwrap(),
                trade.trade_type,
                None,
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("2f80bb1d000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }

        #[tokio::test]
        async fn sqrt_price_limit_x96() {
            let trade = trade_from_route!(ROUTE_0_1, TOKEN0_AMOUNT_100, TradeType::ExactInput);
            let params = quote_call_parameters(
                trade.route(),
                &trade.input_amount().unwrap(),
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U160::from_limbs([0, 0, 1]),
                    use_quoter_v2: false,
                }),
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("f7729d43000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000100000000000000000000000000000000")
            );
            assert_eq!(params.value, U256::ZERO);
        }
    }

    mod single_trade_input_using_quoter_v2 {
        use super::*;
        use alloy_primitives::hex;

        #[tokio::test]
        async fn single_hop_exact_input() {
            let trade = trade_from_route!(ROUTE_0_1, TOKEN0_AMOUNT_100, TradeType::ExactInput);
            let input_amount = trade.input_amount().unwrap();
            let params = quote_call_parameters(
                &trade.swaps[0].route,
                &input_amount,
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U160::ZERO,
                    use_quoter_v2: true,
                }),
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("c6a5026a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000"),
            );
        }

        #[tokio::test]
        async fn single_hop_exact_output() {
            let trade = trade_from_route!(ROUTE_0_1, TOKEN1_AMOUNT_100, TradeType::ExactOutput);
            let output_amount = trade.output_amount().unwrap();
            let params = quote_call_parameters(
                &trade.swaps[0].route,
                &output_amount,
                trade.trade_type,
                Some(QuoteOptions {
                    sqrt_price_limit_x96: U160::ZERO,
                    use_quoter_v2: true,
                }),
            );
            assert_eq!(
                params.calldata.to_vec(),
                hex!("bd21704a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000"),
            );
        }
    }
}
