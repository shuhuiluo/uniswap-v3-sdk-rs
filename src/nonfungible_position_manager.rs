use crate::prelude::{Error, *};
use alloy_primitives::{Bytes, Signature, B256, U256};
use alloy_sol_types::{eip712_domain, Eip712Domain, SolCall, SolStruct};
use num_traits::ToPrimitive;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddLiquiditySpecificOptions {
    Mint(MintSpecificOptions),
    Increase(IncreaseSpecificOptions),
}

/// Options for producing the calldata to add liquidity.
#[derive(Debug, Clone, PartialEq, Eq)]
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
pub struct CollectOptions<Currency0: BaseCurrency, Currency1: BaseCurrency> {
    /// Indicates the ID of the position to collect for.
    pub token_id: U256,
    /// Expected value of tokensOwed0, including as-of-yet-unaccounted-for fees/liquidity value to
    /// be burned
    pub expected_currency_owed0: CurrencyAmount<Currency0>,
    /// Expected value of tokensOwed1, including as-of-yet-unaccounted-for fees/liquidity value to
    /// be burned
    pub expected_currency_owed1: CurrencyAmount<Currency1>,
    /// The account that should receive the tokens.
    pub recipient: Address,
}

pub type NFTPermitValues = IERC721Permit::Permit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NFTPermitData {
    pub domain: Eip712Domain,
    pub values: NFTPermitValues,
}

impl NFTPermitData {
    #[inline]
    #[must_use]
    pub fn eip712_signing_hash(&self) -> B256 {
        self.values.eip712_signing_hash(&self.domain)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NFTPermitOptions {
    pub signature: Signature,
    pub deadline: U256,
    pub spender: Address,
}

/// Options for producing the calldata to exit a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveLiquidityOptions<Currency0: BaseCurrency, Currency1: BaseCurrency> {
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
    pub collect_options: CollectOptions<Currency0, Currency1>,
}

#[inline]
fn encode_create<TP: TickDataProvider>(pool: &Pool<TP>) -> Bytes {
    INonfungiblePositionManager::createAndInitializePoolIfNecessaryCall {
        token0: pool.token0.address(),
        token1: pool.token1.address(),
        fee: pool.fee.into(),
        sqrtPriceX96: pool.sqrt_ratio_x96,
    }
    .abi_encode()
    .into()
}

#[inline]
pub fn create_call_parameters<TP: TickDataProvider>(pool: &Pool<TP>) -> MethodParameters {
    MethodParameters {
        calldata: encode_create(pool),
        value: U256::ZERO,
    }
}

#[inline]
pub fn add_call_parameters<TP: TickDataProvider>(
    position: &mut Position<TP>,
    options: AddLiquidityOptions,
) -> Result<MethodParameters, Error> {
    assert!(position.liquidity > 0, "ZERO_LIQUIDITY");

    let mut calldatas: Vec<Bytes> = Vec::with_capacity(5);

    // get amounts
    let MintAmounts {
        amount0: amount0_desired,
        amount1: amount1_desired,
    } = position.mint_amounts_cached()?;

    // adjust for slippage
    let MintAmounts {
        amount0: amount0_min,
        amount1: amount1_min,
    } = position.mint_amounts_with_slippage(&options.slippage_tolerance)?;

    let deadline = options.deadline;

    // create pool if needed
    if let AddLiquiditySpecificOptions::Mint(opts) = options.specific_opts {
        if opts.create_pool {
            calldatas.push(encode_create(&position.pool));
        }
    }

    // permits if necessary
    if let Some(permit) = options.token0_permit {
        calldatas.push(encode_permit(&position.pool.token0, permit));
    }
    if let Some(permit) = options.token1_permit {
        calldatas.push(encode_permit(&position.pool.token1, permit));
    }

    // mint
    match options.specific_opts {
        AddLiquiditySpecificOptions::Mint(opts) => {
            calldatas.push(
                INonfungiblePositionManager::mintCall {
                    params: INonfungiblePositionManager::MintParams {
                        token0: position.pool.token0.address(),
                        token1: position.pool.token1.address(),
                        fee: position.pool.fee.into(),
                        tickLower: position.tick_lower.to_i24(),
                        tickUpper: position.tick_upper.to_i24(),
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
        let wrapped_value = if position.pool.token0.equals(wrapped) {
            amount0_desired
        } else if position.pool.token1.equals(wrapped) {
            amount1_desired
        } else {
            panic!("NO_WETH");
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

fn encode_collect<Currency0: BaseCurrency, Currency1: BaseCurrency>(
    options: &CollectOptions<Currency0, Currency1>,
) -> Vec<Bytes> {
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
        let token: &Token;
        let token_amount: U256;
        if options.expected_currency_owed0.currency.is_native() {
            eth_amount = U256::from_big_int(options.expected_currency_owed0.quotient());
            token = options.expected_currency_owed1.currency.wrapped();
            token_amount = U256::from_big_int(options.expected_currency_owed1.quotient());
        } else {
            eth_amount = U256::from_big_int(options.expected_currency_owed1.quotient());
            token = options.expected_currency_owed0.currency.wrapped();
            token_amount = U256::from_big_int(options.expected_currency_owed0.quotient());
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

#[inline]
pub fn collect_call_parameters<Currency0: BaseCurrency, Currency1: BaseCurrency>(
    options: &CollectOptions<Currency0, Currency1>,
) -> MethodParameters {
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
#[inline]
pub fn remove_call_parameters<Currency0, Currency1, TP>(
    position: &Position<TP>,
    options: RemoveLiquidityOptions<Currency0, Currency1>,
) -> Result<MethodParameters, Error>
where
    Currency0: BaseCurrency,
    Currency1: BaseCurrency,
    TP: TickDataProvider,
{
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
        position.tick_lower.try_into().unwrap(),
        position.tick_upper.try_into().unwrap(),
    );
    assert!(partial_position.liquidity > 0, "ZERO_LIQUIDITY");

    // slippage-adjusted underlying amounts
    let (amount0_min, amount1_min) =
        partial_position.burn_amounts_with_slippage(&options.slippage_tolerance)?;

    if let Some(permit) = options.permit {
        calldatas.push(
            IERC721Permit::permitCall {
                spender: permit.spender,
                tokenId: token_id,
                deadline: permit.deadline,
                v: permit.signature.v() as u8 + 27,
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
    calldatas.extend(encode_collect(&CollectOptions {
        token_id,
        // add the underlying value to the expected currency already owed
        expected_currency_owed0: expected_currency_owed0.add(&CurrencyAmount::from_raw_amount(
            expected_currency_owed0.currency.clone(),
            amount0_min.to_big_int(),
        )?)?,
        expected_currency_owed1: expected_currency_owed1.add(&CurrencyAmount::from_raw_amount(
            expected_currency_owed1.currency.clone(),
            amount1_min.to_big_int(),
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

#[inline]
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

/// Prepares the parameters for EIP712 signing
///
/// ## Arguments
///
/// * `permit`: The permit values to sign
/// * `position_manager`: The address of the position manager contract
/// * `chain_id`: The chain ID
///
/// ## Returns
///
/// The EIP712 domain and values to sign
///
/// ## Examples
///
/// ```
/// use alloy::signers::{local::PrivateKeySigner, SignerSync};
/// use alloy_primitives::{address, b256, uint, Signature, B256};
/// use alloy_sol_types::SolStruct;
/// use uniswap_v3_sdk::prelude::*;
///
/// let permit = NFTPermitValues {
///     spender: address!("0000000000000000000000000000000000000002"),
///     tokenId: uint!(1_U256),
///     nonce: uint!(1_U256),
///     deadline: uint!(123_U256),
/// };
/// assert_eq!(
///     permit.eip712_type_hash(),
///     b256!("49ecf333e5b8c95c40fdafc95c1ad136e8914a8fb55e9dc8bb01eaa83a2df9ad")
/// );
/// let position_manager = address!("1F98431c8aD98523631AE4a59f267346ea31F984");
/// let data: NFTPermitData = get_permit_data(permit, position_manager, 1);
///
/// // Derive the EIP-712 signing hash.
/// let hash: B256 = data.eip712_signing_hash();
///
/// let signer = PrivateKeySigner::random();
/// let signature: Signature = signer.sign_hash_sync(&hash).unwrap();
/// assert_eq!(
///     signature.recover_address_from_prehash(&hash).unwrap(),
///     signer.address()
/// );
/// ```
#[inline]
#[must_use]
pub const fn get_permit_data(
    permit: NFTPermitValues,
    position_manager: Address,
    chain_id: u64,
) -> NFTPermitData {
    let domain = eip712_domain! {
        name: "Uniswap V3 Positions NFT-V1",
        version: "1",
        chain_id: chain_id,
        verifying_contract: position_manager,
    };
    NFTPermitData {
        domain,
        values: permit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{currency_amount, tests::*};
    use alloy_primitives::{address, hex, uint};
    use once_cell::sync::Lazy;

    const RECIPIENT: Address = address!("0000000000000000000000000000000000000003");
    const SENDER: Address = address!("0000000000000000000000000000000000000004");
    const TOKEN_ID: U256 = uint!(1_U256);
    static SLIPPAGE_TOLERANCE: Lazy<Percent> = Lazy::new(|| Percent::new(1, 100));
    const DEADLINE: U256 = uint!(123_U256);
    static COLLECT_OPTIONS: Lazy<CollectOptions<Token, Token>> = Lazy::new(|| CollectOptions {
        token_id: TOKEN_ID,
        expected_currency_owed0: currency_amount!(TOKEN0, 0),
        expected_currency_owed1: currency_amount!(TOKEN1, 0),
        recipient: RECIPIENT,
    });
    static COLLECT_OPTIONS2: Lazy<CollectOptions<Token, Ether>> = Lazy::new(|| CollectOptions {
        token_id: TOKEN_ID,
        expected_currency_owed0: currency_amount!(TOKEN1, 0),
        expected_currency_owed1: currency_amount!(ETHER, 0),
        recipient: RECIPIENT,
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
        );
        add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: Some(ETHER.clone()),
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
            -FeeAmount::MEDIUM.tick_spacing().as_i32(),
            FeeAmount::MEDIUM.tick_spacing().as_i32(),
        );
        let MethodParameters { calldata, value } = add_call_parameters(
            &mut position,
            AddLiquidityOptions {
                slippage_tolerance: SLIPPAGE_TOLERANCE.clone(),
                deadline: DEADLINE,
                use_native: Some(ETHER.clone()),
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
        let MethodParameters { calldata, value } = collect_call_parameters(&COLLECT_OPTIONS);
        assert_eq!(value, U256::ZERO);
        assert_eq!(
            calldata.to_vec(),
            hex!("fc6f78650000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000ffffffffffffffffffffffffffffffff00000000000000000000000000000000ffffffffffffffffffffffffffffffff")
        );
    }

    #[test]
    fn test_collect_call_parameters_eth() {
        let MethodParameters { calldata, value } = collect_call_parameters(&CollectOptions {
            token_id: TOKEN_ID,
            expected_currency_owed0: currency_amount!(TOKEN1, 0),
            expected_currency_owed1: currency_amount!(ETHER, 0),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
                -FeeAmount::MEDIUM.tick_spacing().as_i32(),
                FeeAmount::MEDIUM.tick_spacing().as_i32(),
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
