use crate::prelude::*;
use alloy_primitives::{Bytes, U256};
use alloy_sol_types::SolCall;
use anyhow::Result;
use uniswap_sdk_core::prelude::*;

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

/// Produces the on-chain method name to call and the hex encoded parameters to pass as arguments
/// for a given trade.
///
/// ## Arguments
///
/// * `trades`: trades to produce call parameters for
/// * `options`: options for the call parameters
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
    let token_in = sample_trade.input_amount()?.currency.wrapped();
    let token_out = sample_trade.output_amount()?.currency.wrapped();

    // All trades should have the same starting and ending token.
    for trade in trades.iter_mut() {
        assert!(
            trade.input_amount()?.currency.wrapped().equals(&token_in),
            "TOKEN_IN_DIFF"
        );
        assert!(
            trade.output_amount()?.currency.wrapped().equals(&token_out),
            "TOKEN_OUT_DIFF"
        );
    }

    let num_swaps = trades.iter().map(|trade| trade.swaps.len()).sum::<usize>();

    let mut calldatas: Vec<Bytes> = Vec::with_capacity(num_swaps + 3);

    let mut total_amount_out = BigInt::zero();
    for trade in trades.iter_mut() {
        total_amount_out += trade
            .minimum_amount_out(slippage_tolerance.clone(), None)?
            .quotient();
    }
    let total_amount_out = big_int_to_u256(total_amount_out);

    // flag for whether a refund needs to happen
    let input_is_native = sample_trade.input_amount()?.currency.is_native();
    let must_refund = input_is_native && sample_trade.trade_type == TradeType::ExactOutput;
    // flags for whether funds should be sent first to the router
    let output_is_native = sample_trade.output_amount()?.currency.is_native();
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
            !sample_trade.input_amount()?.currency.is_native(),
            "NON_TOKEN_PERMIT"
        );
        calldatas.push(encode_permit(
            sample_trade.input_amount()?.currency.wrapped(),
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
                    .abi_encode()
                    .into(),
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
                    .abi_encode()
                    .into(),
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
                    .abi_encode()
                    .into(),
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
                    .abi_encode()
                    .into(),
                });
            }
        }
    }

    // unwrap
    if router_must_custody {
        if output_is_native {
            calldatas.push(encode_unwrap_weth9(
                total_amount_out,
                recipient,
                fee.clone(),
            ));
        } else {
            calldatas.push(encode_sweep_token(
                sample_trade.output_amount()?.currency.address(),
                total_amount_out,
                recipient,
                fee.clone(),
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::{hex, uint};
    use once_cell::sync::Lazy;

    static POOL_0_1: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN0.clone(), TOKEN1.clone()));
    static POOL_1_WETH: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN1.clone(), WETH.clone()));
    static POOL_0_2: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN0.clone(), TOKEN2.clone()));
    static POOL_0_3: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN0.clone(), TOKEN3.clone()));
    static POOL_2_3: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN2.clone(), TOKEN3.clone()));
    static POOL_3_WETH: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN3.clone(), WETH.clone()));
    static POOL_1_3: Lazy<Pool<TickListDataProvider>> =
        Lazy::new(|| make_pool(TOKEN3.clone(), TOKEN1.clone()));

    static SLIPPAGE_TOLERANCE: Lazy<Percent> = Lazy::new(|| Percent::new(1, 100));
    const RECIPIENT: Address = address!("0000000000000000000000000000000000000003");
    const DEADLINE: U256 = uint!(123_U256);
    static SWAP_OPTIONS: Lazy<SwapOptions> = Lazy::new(|| SwapOptions {
        slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
        recipient: RECIPIENT,
        deadline: DEADLINE,
        input_token_permit: None,
        sqrt_price_limit_x96: None,
        fee: None,
    });

    mod single_trade_input {
        use super::*;

        #[test]
        fn single_hop_exact_input() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn single_hop_exact_output() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("db3e2198000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000670000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn multi_hop_exact_input() {
            let trade = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_WETH.clone()],
                    TOKEN0.clone(),
                    WETH.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn multi_hop_exact_output() {
            let trade = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_WETH.clone()],
                    TOKEN0.clone(),
                    WETH.clone(),
                ),
                CurrencyAmount::from_raw_amount(WETH.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_in_exact_input() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], ETHER.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("414bf389000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0x64_U256));
        }

        #[test]
        fn eth_in_exact_output() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], ETHER.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0x67_U256));
        }

        #[test]
        fn eth_out_exact_input() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], TOKEN1.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf3890000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c0000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_out_exact_output() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], TOKEN1.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104db3e21980000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn sqrt_price_limit_x96() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade],
                SwapOptions {
                    sqrt_price_limit_x96: Some(Q128),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000100000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn fee_with_eth_out() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], TOKEN1.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf3890000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000849b2c0a37000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn fee_with_eth_in() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_1_WETH.clone()], ETHER.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 10).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000000002800000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0x0c_U256));
        }

        #[test]
        fn fee() {
            let trade = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }
    }

    mod multiple_trade_input {
        use super::*;

        #[test]
        fn two_single_hop_exact_input() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn one_single_hop_one_multi_hop_exact_input() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_3.clone()], TOKEN0.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_2.clone(), POOL_2_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn two_multi_hop_exact_input() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_2.clone(), POOL_2_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_in_exact_input() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_WETH.clone(), POOL_1_3.clone()],
                    ETHER.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(Ether::on_chain(1), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(Ether::on_chain(1), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f0000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0xc8_U256));
        }

        #[test]
        fn eth_in_exact_output() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_WETH.clone(), POOL_1_3.clone()],
                    ETHER.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0xd0_U256));
        }

        #[test]
        fn eth_out_exact_input() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_3.clone(), POOL_1_WETH.clone()],
                    TOKEN3.clone(),
                    ETHER.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], TOKEN3.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf3890000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_out_exact_output() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_3.clone(), POOL_1_WETH.clone()],
                    TOKEN3.clone(),
                    ETHER.clone(),
                ),
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], TOKEN3.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e21980000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn two_single_hop_exact_output() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN1.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000670000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn one_single_hop_one_multi_hop_exact_output() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_3.clone()], TOKEN0.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_2.clone(), POOL_2_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000670000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn two_multi_hop_exact_output() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_2.clone(), POOL_2_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        #[should_panic(expected = "TOKEN_IN_DIFF")]
        fn different_token_in_fails() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_2_3.clone()], TOKEN2.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN2.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
        }

        #[test]
        #[should_panic(expected = "TOKEN_OUT_DIFF")]
        fn different_token_out_fails() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_3.clone()], TOKEN0.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_WETH.clone()],
                    TOKEN0.clone(),
                    WETH.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
        }

        #[test]
        fn sqrt_price_limit_x96() {
            let trade1 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_0_1.clone()], TOKEN0.clone(), TOKEN1.clone()),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade1, trade2],
                SwapOptions {
                    sqrt_price_limit_x96: Some(Q128),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn fee_with_eth_out() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_3.clone(), POOL_1_WETH.clone()],
                    TOKEN3.clone(),
                    ETHER.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], TOKEN3.clone(), ETHER.clone()),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade1, trade2],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf3890000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000849b2c0a3700000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn fee_with_eth_in() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_1_WETH.clone(), POOL_1_3.clone()],
                    ETHER.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade1, trade2],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d80000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000034000000000000000000000000000000000000000000000000000000000000004200000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a0000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000c800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0xd0_U256));
        }

        #[test]
        fn fee() {
            let trade1 = Trade::from_route(
                Route::new(
                    vec![POOL_0_1.clone(), POOL_1_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(
                    vec![POOL_0_2.clone(), POOL_2_3.clone()],
                    TOKEN0.clone(),
                    TOKEN3.clone(),
                ),
                CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } = swap_call_parameters(
                &mut [trade1, trade2],
                SwapOptions {
                    fee: Some(FeeOptions {
                        fee: Percent::new(5, 1000),
                        recipient: RECIPIENT,
                    }),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003600000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a0000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000be00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }
    }

    mod treade_with_multiple_routes {
        use super::*;

        #[test]
        fn one_single_hop_one_multi_hop_exact_input() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                        Route::new(vec![POOL_0_3.clone()], TOKEN0.clone(), TOKEN3.clone()),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_2.clone(), POOL_2_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                ],
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn two_multi_hop_exact_input() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_1.clone(), POOL_1_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN0.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_2.clone(), POOL_2_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                ],
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_in_exact_input() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_1_WETH.clone(), POOL_1_3.clone()],
                            ETHER.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                        Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                    ),
                ],
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f0000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf389000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0xc8_U256));
        }

        #[test]
        fn eth_in_exact_output() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_1_WETH.clone(), POOL_1_3.clone()],
                            ETHER.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                    ),
                ],
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, uint!(0xd0_U256));
        }

        #[test]
        fn eth_out_exact_input() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_1_3.clone(), POOL_1_WETH.clone()],
                            TOKEN3.clone(),
                            ETHER.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(vec![POOL_3_WETH.clone()], TOKEN3.clone(), ETHER.clone()),
                    ),
                ],
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144c04b8d59000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104414bf3890000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn eth_out_exact_output() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_1_3.clone(), POOL_1_WETH.clone()],
                            TOKEN3.clone(),
                            ETHER.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                        Route::new(vec![POOL_3_WETH.clone()], TOKEN3.clone(), ETHER.clone()),
                    ),
                ],
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000104db3e21980000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn one_single_hop_one_multi_hop_exact_output() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(vec![POOL_0_3.clone()], TOKEN0.clone(), TOKEN3.clone()),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_2.clone(), POOL_2_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                ],
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000104db3e2198000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000670000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }

        #[test]
        fn two_multi_hop_exact_output() {
            let trade = Trade::from_routes(
                vec![
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_1.clone(), POOL_1_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                    (
                        CurrencyAmount::from_raw_amount(TOKEN3.clone(), 100).unwrap(),
                        Route::new(
                            vec![POOL_0_2.clone(), POOL_2_3.clone()],
                            TOKEN0.clone(),
                            TOKEN3.clone(),
                        ),
                    ),
                ],
                TradeType::ExactOutput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000144f28c0498000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }
    }
}
