use crate::prelude::*;
use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use anyhow::Result;
use uniswap_sdk_core::{constants::TradeType, prelude::*};

/// Options for producing the arguments to send calls to the router.
#[derive(Clone, Debug, PartialEq)]
pub struct SwapOptions {
    /// How much the execution price is allowed to move unfavorably for the trade execution price.
    pub slippage_tolerance: Percent,
    /// The account that should receive the output.
    pub recipient: Address,
    /// When the transaction expires, in epoch seconds.
    pub deadline: U256,
    /// The optional permit parameters for spending the input.
    pub input_token_permit: Option<PermitOptions>,
    /// The optional price limit for the trade.
    pub sqrt_price_limit_x96: Option<U256>,
    /// Optional information for taking a fee on output.
    pub fee: Option<FeeOptions>,
}

/// Produces the on-chain method name to call and the hex encoded parameters to pass as arguments for a given trade.
///
/// ## Arguments
///
/// * `trades`: trades to produce call parameters for
/// * `options`: options for the call parameters
///
pub fn swap_call_parameters<TInput: CurrencyTrait, TOutput: CurrencyTrait, P: Clone>(
    trades: &mut [Trade<TInput, TOutput, P>],
    options: SwapOptions,
) -> Result<MethodParameters> {
    let SwapOptions {
        slippage_tolerance,
        recipient,
        deadline,
        input_token_permit,
        sqrt_price_limit_x96,
        fee,
    } = options;
    let mut sample_trade = trades[0].clone();
    let token_in = sample_trade.input_amount()?.meta.currency.wrapped();
    let token_out = sample_trade.output_amount()?.meta.currency.wrapped();

    // All trades should have the same starting and ending token.
    for trade in trades.iter_mut() {
        assert!(
            trade
                .input_amount()?
                .meta
                .currency
                .wrapped()
                .equals(&token_in),
            "TOKEN_IN_DIFF"
        );
        assert!(
            trade
                .output_amount()?
                .meta
                .currency
                .wrapped()
                .equals(&token_out),
            "TOKEN_OUT_DIFF"
        );
    }

    let mut calldatas: Vec<Vec<u8>> = vec![];

    let mut total_amount_out = BigInt::zero();
    for trade in trades.iter_mut() {
        total_amount_out += trade
            .minimum_amount_out(slippage_tolerance.clone(), None)?
            .quotient();
    }
    let total_amount_out = big_int_to_u256(total_amount_out);

    // flag for whether a refund needs to happen
    let input_is_native = sample_trade.input_amount()?.meta.currency.is_native();
    let must_refund = input_is_native && sample_trade.trade_type == TradeType::ExactOutput;
    // flags for whether funds should be send first to the router
    let output_is_native = sample_trade.output_amount()?.meta.currency.is_native();
    let router_must_custody = output_is_native || fee.is_some();

    let mut total_value = BigInt::zero();
    if input_is_native {
        for trade in trades.iter_mut() {
            total_value += trade
                .maximum_amount_in(slippage_tolerance.clone(), None)?
                .quotient();
        }
    }

    // encode permit if necessary
    if let Some(input_token_permit) = input_token_permit {
        assert!(
            !sample_trade.input_amount()?.meta.currency.is_native(),
            "NON_TOKEN_PERMIT"
        );
        calldatas.push(encode_permit(
            sample_trade.input_amount()?.meta.currency.wrapped(),
            input_token_permit,
        ));
    }

    for trade in trades.iter_mut() {
        for Swap {
            route,
            input_amount,
            output_amount,
        } in trade.swaps.clone().iter_mut()
        {
            let amount_in = big_int_to_u256(
                trade
                    .maximum_amount_in(slippage_tolerance.clone(), Some(input_amount.clone()))?
                    .quotient(),
            );
            let amount_out = big_int_to_u256(
                trade
                    .minimum_amount_out(slippage_tolerance.clone(), Some(output_amount.clone()))?
                    .quotient(),
            );

            if route.pools.len() == 1 {
                calldatas.push(match trade.trade_type {
                    TradeType::ExactInput => ISwapRouter::exactInputSingleCall {
                        params: ISwapRouter::ExactInputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            fee: route.pools[0].fee as u32,
                            recipient: if router_must_custody {
                                Address::ZERO
                            } else {
                                recipient
                            },
                            deadline,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                            sqrtPriceLimitX96: sqrt_price_limit_x96.unwrap_or_default(),
                        },
                    }
                    .abi_encode(),
                    TradeType::ExactOutput => ISwapRouter::exactOutputSingleCall {
                        params: ISwapRouter::ExactOutputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            fee: route.pools[0].fee as u32,
                            recipient: if router_must_custody {
                                Address::ZERO
                            } else {
                                recipient
                            },
                            deadline,
                            amountOut: amount_out,
                            amountInMaximum: amount_in,
                            sqrtPriceLimitX96: sqrt_price_limit_x96.unwrap_or_default(),
                        },
                    }
                    .abi_encode(),
                });
            } else {
                assert!(sqrt_price_limit_x96.is_none(), "MULTIHOP_PRICE_LIMIT");

                let path = encode_route_to_path(route, trade.trade_type == TradeType::ExactOutput);

                calldatas.push(match trade.trade_type {
                    TradeType::ExactInput => ISwapRouter::exactInputCall {
                        params: ISwapRouter::ExactInputParams {
                            path,
                            recipient: if router_must_custody {
                                Address::ZERO
                            } else {
                                recipient
                            },
                            deadline,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                        },
                    }
                    .abi_encode(),
                    TradeType::ExactOutput => ISwapRouter::exactOutputCall {
                        params: ISwapRouter::ExactOutputParams {
                            path,
                            recipient: if router_must_custody {
                                Address::ZERO
                            } else {
                                recipient
                            },
                            deadline,
                            amountOut: amount_out,
                            amountInMaximum: amount_in,
                        },
                    }
                    .abi_encode(),
                });
            }
        }
    }

    // unwrap
    if router_must_custody {
        if let Some(fee) = fee {
            if output_is_native {
                calldatas.push(encode_unwrap_weth9(
                    total_amount_out,
                    recipient,
                    Some(fee.clone()),
                ));
            } else {
                calldatas.push(encode_sweep_token(
                    sample_trade.output_amount()?.meta.currency.address(),
                    total_amount_out,
                    recipient,
                    Some(fee.clone()),
                ));
            }
        } else {
            calldatas.push(encode_unwrap_weth9(total_amount_out, recipient, None));
        }
    }

    // refund
    if must_refund {
        calldatas.push(encode_refund_eth());
    }

    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value: big_int_to_u256(total_value),
    })
}
