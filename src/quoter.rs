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
    route: Route<TInput, TOutput, P>,
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
