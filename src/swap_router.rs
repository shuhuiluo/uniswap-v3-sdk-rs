use crate::prelude::{Error, *};
use alloy_primitives::{Bytes, U160, U256};
use alloy_sol_types::SolCall;
use uniswap_sdk_core::prelude::*;

/// Options for producing the arguments to send calls to the router.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwapOptions {
    /// How much the execution price is allowed to move unfavorably for the trade execution price.
    pub slippage_tolerance: Percent,
    /// The account that should receive the output.
    pub recipient: Address,
    /// The optional permit parameters for spending the input.
    pub input_token_permit: Option<PermitOptions>,
    /// The optional price limit for the trade.
    pub sqrt_price_limit_x96: Option<U160>,
    /// Optional information for taking a fee on output.
    pub fee: Option<FeeOptions>,
}

/// Produces the on-chain method name to call and the hex encoded parameters to pass as arguments
/// for a given trade on [`SwapRouter02`](https://github.com/Uniswap/swap-router-contracts/blob/main/contracts/SwapRouter02.sol).
///
/// ## Notes
///
/// The check on deadline is delegated to [`multicall`](https://github.com/Uniswap/swap-router-contracts/blob/main/contracts/interfaces/IMulticallExtended.sol#L15).
///
/// ## Arguments
///
/// * `trades`: trades to produce call parameters for
/// * `options`: options for the call parameters
#[inline]
pub fn swap_call_parameters<TInput, TOutput, TP>(
    trades: &mut [Trade<TInput, TOutput, TP>],
    options: SwapOptions,
) -> Result<MethodParameters, Error>
where
    TInput: BaseCurrency,
    TOutput: BaseCurrency,
    TP: TickDataProvider,
{
    let SwapOptions {
        slippage_tolerance,
        recipient,
        input_token_permit,
        sqrt_price_limit_x96,
        fee,
    } = options;
    let sample_trade = &trades[0];
    let input_currency = sample_trade.input_currency();
    let token_in = input_currency.wrapped();
    let input_is_native = input_currency.is_native();
    let output_currency = sample_trade.output_currency();
    let token_out = output_currency.wrapped();
    let output_currency_address = output_currency.address();
    let output_is_native = output_currency.is_native();
    let trade_type = sample_trade.trade_type;

    // All trades should have the same starting and ending token.
    for trade in trades.iter() {
        assert!(
            trade.input_currency().wrapped().equals(token_in),
            "TOKEN_IN_DIFF"
        );
        assert!(
            trade.output_currency().wrapped().equals(token_out),
            "TOKEN_OUT_DIFF"
        );
    }

    let num_swaps = trades.iter().map(|trade| trade.swaps.len()).sum::<usize>();

    let mut calldatas: Vec<Bytes> = Vec::with_capacity(num_swaps + 3);

    // encode permit if necessary
    if let Some(input_token_permit) = input_token_permit {
        assert!(!input_is_native, "NON_TOKEN_PERMIT");
        calldatas.push(encode_permit(token_in, input_token_permit));
    }

    let mut total_amount_out = BigInt::ZERO;
    for trade in trades.iter_mut() {
        total_amount_out += trade
            .minimum_amount_out_cached(slippage_tolerance.clone(), None)?
            .quotient();
    }
    let total_amount_out = U256::from_big_int(total_amount_out);

    // flag for whether a refund needs to happen
    let must_refund = input_is_native && trade_type == TradeType::ExactOutput;
    // flags for whether funds should be sent first to the router
    let router_must_custody = output_is_native || fee.is_some();

    let mut total_value = BigInt::ZERO;
    if input_is_native {
        for trade in trades.iter_mut() {
            total_value += trade
                .maximum_amount_in_cached(slippage_tolerance.clone(), None)?
                .quotient();
        }
    }
    let intermediate_recipient = if router_must_custody {
        Address::ZERO
    } else {
        recipient
    };

    for trade in trades.iter() {
        for Swap {
            route,
            input_amount,
            output_amount,
        } in &trade.swaps
        {
            let amount_in = U256::from_big_int(
                trade
                    .maximum_amount_in(slippage_tolerance.clone(), Some(input_amount.clone()))?
                    .quotient(),
            );
            let amount_out = U256::from_big_int(
                trade
                    .minimum_amount_out(slippage_tolerance.clone(), Some(output_amount.clone()))?
                    .quotient(),
            );

            if route.pools.len() == 1 {
                calldatas.push(match trade.trade_type {
                    TradeType::ExactInput => IV3SwapRouter::exactInputSingleCall {
                        params: IV3SwapRouter::ExactInputSingleParams {
                            tokenIn: route.input.wrapped().address(),
                            tokenOut: route.output.wrapped().address(),
                            fee: route.pools[0].fee.into(),
                            recipient: intermediate_recipient,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                            sqrtPriceLimitX96: sqrt_price_limit_x96.unwrap_or_default(),
                        },
                    }
                    .abi_encode()
                    .into(),
                    TradeType::ExactOutput => IV3SwapRouter::exactOutputSingleCall {
                        params: IV3SwapRouter::ExactOutputSingleParams {
                            tokenIn: route.input.wrapped().address(),
                            tokenOut: route.output.wrapped().address(),
                            fee: route.pools[0].fee.into(),
                            recipient: intermediate_recipient,
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
                    TradeType::ExactInput => IV3SwapRouter::exactInputCall {
                        params: IV3SwapRouter::ExactInputParams {
                            path,
                            recipient: intermediate_recipient,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                        },
                    }
                    .abi_encode()
                    .into(),
                    TradeType::ExactOutput => IV3SwapRouter::exactOutputCall {
                        params: IV3SwapRouter::ExactOutputParams {
                            path,
                            recipient: intermediate_recipient,
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
            calldatas.push(encode_unwrap_weth9(total_amount_out, recipient, fee));
        } else {
            calldatas.push(encode_sweep_token(
                output_currency_address,
                total_amount_out,
                recipient,
                fee,
            ));
        }
    }

    // refund
    if must_refund {
        calldatas.push(encode_refund_eth());
    }

    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value: U256::from_big_int(total_value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::{address, hex, uint};
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
    static SWAP_OPTIONS: Lazy<SwapOptions> = Lazy::new(|| SwapOptions {
        slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
        recipient: RECIPIENT,
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
            assert_eq!(calldata.to_vec(), hex!("04e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("5023b4df000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000670000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("09b81346000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("04e45aaf000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c0000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e45023b4df0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
                    sqrt_price_limit_x96: Some(U160::from_limbs([0, 0, 1])),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("04e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000100000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000849b2c0a37000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d80000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000180000000000000000000000000000000000000000000000000000000000000026000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let trade2 = Trade::from_route(
                Route::new(vec![POOL_3_WETH.clone()], ETHER.clone(), TOKEN3.clone()),
                CurrencyAmount::from_raw_amount(ETHER.clone(), 100).unwrap(),
                TradeType::ExactInput,
            )
            .unwrap();
            let MethodParameters { calldata, value } =
                swap_call_parameters(&mut [trade1, trade2], SWAP_OPTIONS.clone()).unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f0000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e0000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e0000000000000000000000000000000000000000000000000000000000000012409b81346000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a0000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
                    sqrt_price_limit_x96: Some(U160::from_limbs([0, 0, 1])),
                    ..SWAP_OPTIONS.clone()
                },
            )
            .unwrap();
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000849b2c0a3700000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000001e0000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000003e0000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a0000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000c800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a4e0e189a0000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000be00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000001000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f0000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e0000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e00000000000000000000000000000000000000000000000000000000000000124b858183f0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000005f00000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e404e45aaf0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000061000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000002e0000000000000000000000000000000000000000000000000000000000000012409b81346000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000690000000000000000000000000000000000000000000000000000000000000042c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb800000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e45023b4df0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000c8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000e45023b4df000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000067000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
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
            assert_eq!(calldata.to_vec(), hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001a0000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000002000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012409b813460000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000006900000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000004000bb80000000000000000000000000000000000000003000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
            assert_eq!(value, U256::ZERO);
        }
    }
}
