use super::abi::INonfungiblePositionManager;
use crate::{
    prelude::{
        encode_multicall, encode_permit, MethodParameters, MintAmounts, PermitOptions, Pool,
        Position,
    },
    utils::{big_int_to_u256, u256_to_big_int},
};
use alloy_primitives::{Address, Signature, U256};
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
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectOptions {
    /// Indicates the ID of the position to collect for.
    pub token_id: U256,
    /// Expected value of tokensOwed0, including as-of-yet-unaccounted-for fees/liquidity value to be burned
    pub expected_currency_owed0: CurrencyAmount<Currency>,
    /// Expected value of tokensOwed1, including as-of-yet-unaccounted-for fees/liquidity value to be burned
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
    /// The optional permit of the token ID being exited, in case the exit transaction is being sent by an account that does not own the NFT
    pub permit: Option<NFTPermitOptions>,
    /// Parameters to be passed on to collect
    pub collect_options: CollectOptions,
}

fn encode_create(pool: &Pool) -> Vec<u8> {
    INonfungiblePositionManager::createAndInitializePoolIfNecessaryCall {
        token0: pool.token0.address(),
        token1: pool.token1.address(),
        fee: pool.fee as u32,
        sqrtPriceX96: pool.sqrt_ratio_x96,
    }
    .abi_encode()
}

pub fn create_call_parameters(pool: &Pool) -> MethodParameters {
    MethodParameters {
        calldata: encode_create(pool),
        value: U256::ZERO,
    }
}

pub fn add_call_parameters(
    position: &mut Position,
    options: AddLiquidityOptions,
) -> Result<MethodParameters> {
    assert!(position.liquidity > 0, "ZERO_LIQUIDITY");

    let mut calldatas: Vec<Vec<u8>> = vec![];

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
                .abi_encode(),
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
                .abi_encode(),
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
            // TODO: refund eth
            // calldatas.push(encode_refund_eth());
        }

        value = wrapped_value;
    }
    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value,
    })
}

fn encode_collect(options: CollectOptions) -> Vec<Vec<u8>> {
    let mut calldatas: Vec<Vec<u8>> = vec![];

    let involves_eth = options.expected_currency_owed0.meta.currency.is_native()
        || options.expected_currency_owed1.meta.currency.is_native();

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
        .abi_encode(),
    );

    if involves_eth {
        let _eth_amount: U256;
        let _token: Token;
        let _token_amount: U256;
        if options.expected_currency_owed0.meta.currency.is_native() {
            _eth_amount = big_int_to_u256(options.expected_currency_owed0.quotient());
            _token = options.expected_currency_owed1.meta.currency.wrapped();
            _token_amount = big_int_to_u256(options.expected_currency_owed1.quotient());
        } else {
            _eth_amount = big_int_to_u256(options.expected_currency_owed1.quotient());
            _token = options.expected_currency_owed0.meta.currency.wrapped();
            _token_amount = big_int_to_u256(options.expected_currency_owed0.quotient());
        }

        // TODO: unwrap weth
        // calldatas.push(encode_unwrap_weth9(eth_amount, options.recipient).abi_encode());
        // calldatas.push(encode_sweep_token(token, token_amount, options.recipient).abi_encode());
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
///
pub fn remove_call_parameters(
    position: &Position,
    options: RemoveLiquidityOptions,
) -> Result<MethodParameters> {
    let mut calldatas: Vec<Vec<u8>> = vec![];

    let deadline = options.deadline;
    let token_id = options.token_id;

    // construct a partial position with a percentage of liquidity
    let mut partial_position = Position::new(
        position.pool.clone(),
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
            .abi_encode(),
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
        .abi_encode(),
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
            expected_currency_owed0.meta.currency.clone(),
            u256_to_big_int(amount0_min),
        )?)?,
        expected_currency_owed1: expected_currency_owed1.add(&CurrencyAmount::from_raw_amount(
            expected_currency_owed1.meta.currency.clone(),
            u256_to_big_int(amount1_min),
        )?)?,
        recipient: options.collect_options.recipient,
    }));

    if options.liquidity_percentage == Percent::new(1, 1) {
        if options.burn_token {
            calldatas
                .push(INonfungiblePositionManager::burnCall { tokenId: token_id }.abi_encode());
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
        calldata,
        value: U256::ZERO,
    }
}
