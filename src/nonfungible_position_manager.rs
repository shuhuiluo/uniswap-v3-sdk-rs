use crate::prelude::*;
use alloy_primitives::{Bytes, Signature, U256};
use alloy_sol_types::SolCall;
use anyhow::Result;
use uniswap_sdk_core::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintSpecificOptions {
    /// The account that should receive the minted NFT.
    pub recipient: Address,
    /// Creates pool if not initialized before mint.
    pub create_pool: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncreaseSpecificOptions {
    /// Indicates the ID of the position to increase liquidity for.
    pub token_id: U256,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddLiquiditySpecificOptions {
    Mint(MintSpecificOptions),
    Increase(IncreaseSpecificOptions),
}

/// Options for producing the calldata to add liquidity.
#[derive(Debug, Clone, PartialEq)]
pub struct AddLiquidityOptions {
    /// How much the pool price is allowed to move.
    pub slippage_tolerance: Percent,
    /// When the transaction expires, in epoch seconds.
    pub deadline: U256,
    /// Whether to spend ether. If true, one of the pool tokens must be WETH, by default false
    pub use_native: Option<Ether>,
    /// The optional permit parameters for spending token0
    pub token0_permit: Option<PermitOptions>,
    /// The optional permit parameters for spending token1
    pub token1_permit: Option<PermitOptions>,
    /// [`MintSpecificOptions`] or [`IncreaseSpecificOptions`]
    pub specific_opts: AddLiquiditySpecificOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeTransferOptions {
    /// The account sending the NFT.
    pub sender: Address,
    /// The account that should receive the NFT.
    pub recipient: Address,
    /// The id of the token being sent.
    pub token_id: U256,
    /// The optional parameter that passes data to the `onERC721Received` call for the staker
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectOptions {
    /// Indicates the ID of the position to collect for.
    pub token_id: U256,
    /// Expected value of tokensOwed0, including as-of-yet-unaccounted-for fees/liquidity value to
    /// be burned
    pub expected_currency_owed0: CurrencyAmount<Currency>,
    /// Expected value of tokensOwed1, including as-of-yet-unaccounted-for fees/liquidity value to
    /// be burned
    pub expected_currency_owed1: CurrencyAmount<Currency>,
    /// The account that should receive the tokens.
    pub recipient: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NFTPermitOptions {
    pub signature: Signature,
    pub deadline: U256,
    pub spender: Address,
}

/// Options for producing the calldata to exit a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveLiquidityOptions {
    /// The ID of the token to exit
    pub token_id: U256,
    /// The percentage of position liquidity to exit.
    pub liquidity_percentage: Percent,
    /// How much the pool price is allowed to move.
    pub slippage_tolerance: Percent,
    /// When the transaction expires, in epoch seconds.
    pub deadline: U256,
    /// Whether the NFT should be burned if the entire position is being exited, by default false.
    pub burn_token: bool,
    /// The optional permit of the token ID being exited, in case the exit transaction is being
    /// sent by an account that does not own the NFT
    pub permit: Option<NFTPermitOptions>,
    /// Parameters to be passed on to collect
    pub collect_options: CollectOptions,
}

fn encode_create<P>(pool: &Pool<P>) -> Bytes {
    INonfungiblePositionManager::createAndInitializePoolIfNecessaryCall {
        token0: pool.token0.address(),
        token1: pool.token1.address(),
        fee: pool.fee as u32,
        sqrtPriceX96: pool.sqrt_ratio_x96,
    }
    .abi_encode()
    .into()
}

pub fn create_call_parameters<P>(pool: &Pool<P>) -> MethodParameters {
    MethodParameters {
        calldata: encode_create(pool),
        value: U256::ZERO,
    }
}

pub fn add_call_parameters<P>(
    position: &mut Position<P>,
    options: AddLiquidityOptions,
) -> Result<MethodParameters> {
    assert!(position.liquidity > 0, "ZERO_LIQUIDITY");

    let mut calldatas: Vec<Bytes> = Vec::with_capacity(5);

    // get amounts
    let MintAmounts {
        amount0: amount0_desired,
        amount1: amount1_desired,
    } = position.mint_amounts()?;

    // adjust for slippage
    let MintAmounts {
        amount0: amount0_min,
        amount1: amount1_min,
    } = position.mint_amounts_with_slippage(&options.slippage_tolerance)?;

    let deadline = options.deadline;

    // create pool if needed
    if let AddLiquiditySpecificOptions::Mint(opts) = options.specific_opts {
        if opts.create_pool {
            calldatas.push(encode_create(&position.pool))
        }
    }

    // permits if necessary
    if let Some(permit) = options.token0_permit {
        calldatas.push(encode_permit(position.pool.token0.clone(), permit));
    }
    if let Some(permit) = options.token1_permit {
        calldatas.push(encode_permit(position.pool.token1.clone(), permit));
    }

    // mint
    match options.specific_opts {
        AddLiquiditySpecificOptions::Mint(opts) => {
            calldatas.push(
                INonfungiblePositionManager::mintCall {
                    params: INonfungiblePositionManager::MintParams {
                        token0: position.pool.token0.address(),
                        token1: position.pool.token1.address(),
                        fee: position.pool.fee as u32,
                        tickLower: position.tick_lower,
                        tickUpper: position.tick_upper,
                        amount0Desired: amount0_desired,
                        amount1Desired: amount1_desired,
                        amount0Min: amount0_min,
                        amount1Min: amount1_min,
                        recipient: opts.recipient,
                        deadline,
                    },
                }
                .abi_encode()
                .into(),
            );
        }
        AddLiquiditySpecificOptions::Increase(opts) => {
            calldatas.push(
                INonfungiblePositionManager::increaseLiquidityCall {
                    params: INonfungiblePositionManager::IncreaseLiquidityParams {
                        tokenId: opts.token_id,
                        amount0Desired: amount0_desired,
                        amount1Desired: amount1_desired,
                        amount0Min: amount0_min,
                        amount1Min: amount1_min,
                        deadline,
                    },
                }
                .abi_encode()
                .into(),
            );
        }
    }

    let mut value = U256::ZERO;

    if let Some(ether) = options.use_native {
        let wrapped = ether.wrapped();
        assert!(
            position.pool.token0.equals(&wrapped) || position.pool.token1.equals(&wrapped),
            "NO_WETH"
        );

        let wrapped_value = if position.pool.token0.equals(&wrapped) {
            amount0_desired
        } else {
            amount1_desired
        };

        // we only need to refund if we're actually sending ETH
        if wrapped_value > U256::ZERO {
            calldatas.push(encode_refund_eth());
        }

        value = wrapped_value;
    }
    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value,
    })
}

fn encode_collect(options: CollectOptions) -> Vec<Bytes> {
    let mut calldatas: Vec<Bytes> = Vec::with_capacity(3);

    let involves_eth = options.expected_currency_owed0.currency.is_native()
        || options.expected_currency_owed1.currency.is_native();

    // collect
    calldatas.push(
        INonfungiblePositionManager::collectCall {
            params: INonfungiblePositionManager::CollectParams {
                tokenId: options.token_id,
                recipient: if involves_eth {
                    Address::ZERO
                } else {
                    options.recipient
                },
                amount0Max: u128::MAX,
                amount1Max: u128::MAX,
            },
        }
        .abi_encode()
        .into(),
    );

    if involves_eth {
        let eth_amount: U256;
        let token: Token;
        let token_amount: U256;
        if options.expected_currency_owed0.currency.is_native() {
            eth_amount = big_int_to_u256(options.expected_currency_owed0.quotient());
            token = options.expected_currency_owed1.currency.wrapped();
            token_amount = big_int_to_u256(options.expected_currency_owed1.quotient());
        } else {
            eth_amount = big_int_to_u256(options.expected_currency_owed1.quotient());
            token = options.expected_currency_owed0.currency.wrapped();
            token_amount = big_int_to_u256(options.expected_currency_owed0.quotient());
        }

        calldatas.push(encode_unwrap_weth9(eth_amount, options.recipient, None));
        calldatas.push(encode_sweep_token(
            token.address(),
            token_amount,
            options.recipient,
            None,
        ));
    }
    calldatas
}

pub fn collect_call_parameters(options: CollectOptions) -> MethodParameters {
    let calldatas = encode_collect(options);

    MethodParameters {
        calldata: encode_multicall(calldatas),
        value: U256::ZERO,
    }
}

/// Produces the calldata for completely or partially exiting a position
///
/// ## Arguments
///
/// * `position`: The position to exit
/// * `options`: Additional information necessary for generating the calldata
pub fn remove_call_parameters<P>(
    position: &Position<P>,
    options: RemoveLiquidityOptions,
) -> Result<MethodParameters> {
    let mut calldatas: Vec<Bytes> = Vec::with_capacity(6);

    let deadline = options.deadline;
    let token_id = options.token_id;

    // construct a partial position with a percentage of liquidity
    let partial_position = Position::new(
        Pool::new(
            position.pool.token0.clone(),
            position.pool.token1.clone(),
            position.pool.fee,
            position.pool.sqrt_ratio_x96,
            position.pool.liquidity,
        )?,
        (options.liquidity_percentage.clone() * Percent::new(position.liquidity, 1))
            .quotient()
            .to_u128()
            .unwrap(),
        position.tick_lower,
        position.tick_upper,
    );
    assert!(partial_position.liquidity > 0, "ZERO_LIQUIDITY");

    // slippage-adjusted underlying amounts
    let (amount0_min, amount1_min) =
        partial_position.burn_amounts_with_slippage(&options.slippage_tolerance)?;

    if let Some(permit) = options.permit {
        calldatas.push(
            INonfungiblePositionManager::permitCall {
                spender: permit.spender,
                tokenId: token_id,
                deadline: permit.deadline,
                v: permit.signature.v().y_parity_byte(),
                r: permit.signature.r().into(),
                s: permit.signature.s().into(),
            }
            .abi_encode()
            .into(),
        );
    };

    // remove liquidity
    calldatas.push(
        INonfungiblePositionManager::decreaseLiquidityCall {
            params: INonfungiblePositionManager::DecreaseLiquidityParams {
                tokenId: token_id,
                liquidity: partial_position.liquidity,
                amount0Min: amount0_min,
                amount1Min: amount1_min,
                deadline,
            },
        }
        .abi_encode()
        .into(),
    );

    let CollectOptions {
        expected_currency_owed0,
        expected_currency_owed1,
        ..
    } = options.collect_options;
    calldatas.extend(encode_collect(CollectOptions {
        token_id,
        // add the underlying value to the expected currency already owed
        expected_currency_owed0: expected_currency_owed0.add(&CurrencyAmount::from_raw_amount(
            expected_currency_owed0.currency.clone(),
            u256_to_big_int(amount0_min),
        )?)?,
        expected_currency_owed1: expected_currency_owed1.add(&CurrencyAmount::from_raw_amount(
            expected_currency_owed1.currency.clone(),
            u256_to_big_int(amount1_min),
        )?)?,
        recipient: options.collect_options.recipient,
    }));

    if options.liquidity_percentage == Percent::new(1, 1) {
        if options.burn_token {
            calldatas.push(
                INonfungiblePositionManager::burnCall { tokenId: token_id }
                    .abi_encode()
                    .into(),
            );
        }
    } else {
        assert!(!options.burn_token, "CANNOT_BURN");
    }

    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value: U256::ZERO,
    })
}

pub fn safe_transfer_from_parameters(options: SafeTransferOptions) -> MethodParameters {
    let calldata = if options.data.is_empty() {
        INonfungiblePositionManager::safeTransferFrom_0Call {
            from: options.sender,
            to: options.recipient,
            tokenId: options.token_id,
        }
        .abi_encode()
    } else {
        INonfungiblePositionManager::safeTransferFrom_1Call {
            from: options.sender,
            to: options.recipient,
            tokenId: options.token_id,
            data: options.data,
        }
        .abi_encode()
    };
    MethodParameters {
        calldata: calldata.into(),
        value: U256::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::{hex, uint};
    use once_cell::sync::Lazy;

    const RECIPIENT: Address = address!("0000000000000000000000000000000000000003");
    const SENDER: Address = address!("0000000000000000000000000000000000000004");
    const TOKEN_ID: U256 = uint!(1_U256);
    static SLIPPAGE_TOLERANCE: Lazy<Percent> = Lazy::new(|| Percent::new(1, 100));
    const DEADLINE: U256 = uint!(123_U256);
    static COLLECT_OPTIONS: Lazy<CollectOptions> = Lazy::new(|| CollectOptions {
        token_id: TOKEN_ID,
        expected_currency_owed0: CurrencyAmount::from_raw_amount(
            Currency::Token(TOKEN0.clone()),
            0,
        )
        .unwrap(),
        expected_currency_owed1: CurrencyAmount::from_raw_amount(
            Currency::Token(TOKEN1.clone()),
            0,
        )
        .unwrap(),
        recipient: RECIPIENT,
    });
    static COLLECT_OPTIONS2: Lazy<CollectOptions> = Lazy::new(|| {
        let eth_amount =
            CurrencyAmount::from_raw_amount(Currency::NativeCurrency(Ether::on_chain(1)), 0)
                .unwrap();
        let token_amount =
            CurrencyAmount::from_raw_amount(Currency::Token(TOKEN1.clone()), 0).unwrap();
        let condition = POOL_1_WETH.token0.equals(&TOKEN1.clone());
        CollectOptions {
            token_id: TOKEN_ID,
            expected_currency_owed0: if condition {
                token_amount.clone()
            } else {
                eth_amount.clone()
            },
            expected_currency_owed1: if condition {
                eth_amount.clone()
            } else {
                token_amount.clone()
            },
            recipient: RECIPIENT,
        }
    });

    #[test]
    fn test_create_call_parameters() {
        let MethodParameters { calldata, value } = create_call_parameters(&POOL_0_1);
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("13ead562000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb80000000000000000000000000000000000000001000000000000000000000000")
        );
    }

    #[test]
    #[should_panic(expected = "ZERO_LIQUIDITY")]
    fn test_add_call_parameters_zero_liquidity() {
        let mut position = Position::new(
            POOL_0_1.clone(),
            0,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: None,
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
                    recipient: RECIPIENT,
                    create_pool: false,
                }),
            },
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "NO_WETH")]
    fn test_add_call_parameters_no_weth() {
        let mut position = Position::new(
            POOL_0_1.clone(),
            1,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: Some(Ether::on_chain(1)),
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
                    recipient: RECIPIENT,
                    create_pool: false,
                }),
            },
        )
        .unwrap();
    }

    #[test]
    fn test_add_call_parameters_mint() {
        let mut position = Position::new(
            POOL_0_1.clone(),
            1,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        let MethodParameters { calldata, value } = add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: None,
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
                    recipient: RECIPIENT,
                    create_pool: false,
                }),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("88316456000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc4000000000000000000000000000000000000000000000000000000000000003c00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b")
        );
    }

    #[test]
    fn test_add_call_parameters_increase() {
        let mut position = Position::new(
            POOL_0_1.clone(),
            1,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        let MethodParameters { calldata, value } = add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: None,
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Increase(IncreaseSpecificOptions {
                    token_id: TOKEN_ID,
                }),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("219f5d1700000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b")
        );
    }

    #[test]
    fn test_add_call_parameters_create_pool() {
        let mut position = Position::new(
            POOL_0_1.clone(),
            1,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        let MethodParameters { calldata, value } = add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: None,
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
                    recipient: RECIPIENT,
                    create_pool: true,
                }),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d80000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000008413ead562000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000016488316456000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000bb8ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc4000000000000000000000000000000000000000000000000000000000000003c00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_add_call_parameters_use_native() {
        let mut position = Position::new(
            POOL_1_WETH.clone(),
            1,
            -FeeAmount::MEDIUM.tick_spacing(),
            FeeAmount::MEDIUM.tick_spacing(),
        );
        let MethodParameters { calldata, value } = add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: Some(Ether::on_chain(1)),
                token0_permit: None,
                token1_permit: None,
                specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
                    recipient: RECIPIENT,
                    create_pool: false,
                }),
            },
        )
        .unwrap();
        assert_eq!(value, uint!(1_U256));
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000164883164560000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000bb8ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc4000000000000000000000000000000000000000000000000000000000000003c00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000007b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000412210e8a00000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_collect_call_parameters() {
        let MethodParameters { calldata, value } = collect_call_parameters(COLLECT_OPTIONS.clone());
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff")
        );
    }

    #[test]
    fn test_collect_call_parameters_eth() {
        let MethodParameters { calldata, value } = collect_call_parameters(CollectOptions {
            token_id: TOKEN_ID,
            expected_currency_owed0: CurrencyAmount::from_raw_amount(
                Currency::Token(TOKEN1.clone()),
                0,
            )
            .unwrap(),
            expected_currency_owed1: CurrencyAmount::from_raw_amount(
                Currency::NativeCurrency(Ether::on_chain(1)),
                0,
            )
            .unwrap(),
            recipient: RECIPIENT,
        });
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000001a00000000000000000000000000000000000000000000000000000000000000084fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064df2ab5bb00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    #[should_panic(expected = "ZERO_LIQUIDITY")]
    fn test_remove_call_parameters_zero_liquidity() {
        remove_call_parameters(
            &Position::new(
                POOL_0_1.clone(),
                0,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 1),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS.clone(),
            },
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "ZERO_LIQUIDITY")]
    fn test_remove_call_parameters_small_percentage() {
        remove_call_parameters(
            &Position::new(
                POOL_0_1.clone(),
                1,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 100),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS.clone(),
            },
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "CANNOT_BURN")]
    fn test_remove_call_parameters_bad_burn() {
        remove_call_parameters(
            &Position::new(
                POOL_0_1.clone(),
                50,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(99, 100),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: true,
                permit: None,
                collect_options: COLLECT_OPTIONS.clone(),
            },
        )
        .unwrap();
    }

    #[test]
    fn test_remove_call_parameters_burn() {
        let MethodParameters { calldata, value } = remove_call_parameters(
            &Position::new(
                POOL_0_1.clone(),
                100,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 1),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS.clone(),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000a40c49ccbe0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000084fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_remove_call_parameters_partial() {
        let MethodParameters { calldata, value } = remove_call_parameters(
            &Position::new(
                POOL_0_1.clone(),
                100,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 2),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS.clone(),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d8000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000a40c49ccbe0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000084fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_remove_call_parameters_eth() {
        let MethodParameters { calldata, value } = remove_call_parameters(
            &Position::new(
                POOL_1_WETH.clone(),
                100,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 1),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS2.clone(),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d80000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000000a40c49ccbe0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000084fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064df2ab5bb00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_remove_call_parameters_partial_eth() {
        let MethodParameters { calldata, value } = remove_call_parameters(
            &Position::new(
                POOL_1_WETH.clone(),
                100,
                -FeeAmount::MEDIUM.tick_spacing(),
                FeeAmount::MEDIUM.tick_spacing(),
            ),
            RemoveLiquidityOptions {
                token_id: TOKEN_ID,
                liquidity_percentage: Percent::new(1, 2),
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                burn_token: false,
                permit: None,
                collect_options: COLLECT_OPTIONS2.clone(),
            },
        )
        .unwrap();
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("ac9650d80000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000000a40c49ccbe0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000003200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000084fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004449404b7c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064df2ab5bb00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn test_safe_transfer_from_parameters_no_data() {
        let MethodParameters { calldata, value } =
            safe_transfer_from_parameters(SafeTransferOptions {
                sender: SENDER,
                recipient: RECIPIENT,
                token_id: TOKEN_ID,
                data: Bytes::default(),
            });
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("42842e0e000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn test_safe_transfer_from_parameters_data() {
        let MethodParameters { calldata, value } =
            safe_transfer_from_parameters(SafeTransferOptions {
                sender: SENDER,
                recipient: RECIPIENT,
                token_id: TOKEN_ID,
                data: hex!("0000000000000000000000000000000000009004").into(),
            });
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("b88d4fde000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000000000009004000000000000000000000000")
        );
    }
}
