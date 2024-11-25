use crate::constants::{FeeAmount, POOL_INIT_CODE_HASH};
use alloy_primitives::{aliases::U24, b256, keccak256, Address, B256};
use alloy_sol_types::SolValue;
use uniswap_sdk_core::prelude::{
    compute_zksync_create2_address::compute_zksync_create2_address, ChainId,
};

/// Computes a pool address
///
/// ## Arguments
///
/// * `factory`: The Uniswap V3 factory address
/// * `token_a`: The first token of the pair, irrespective of sort order
/// * `token_b`: The second token of the pair, irrespective of sort order
/// * `fee`: The fee tier of the pool
/// * `init_code_hash_manual_override`: Override the init code hash used to compute the pool address
///   if necessary
///
/// ## Returns
///
/// The computed pool address
///
/// ## Examples
///
/// ```
/// use alloy_primitives::{address, Address};
/// use uniswap_v3_sdk::prelude::*;
///
/// const FACTORY_ADDRESS: Address = address!("1111111111111111111111111111111111111111");
/// const USDC_ADDRESS: Address = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
/// const DAI_ADDRESS: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
/// let result = compute_pool_address(
///     FACTORY_ADDRESS,
///     USDC_ADDRESS,
///     DAI_ADDRESS,
///     FeeAmount::LOW,
///     None,
///     None,
/// );
/// assert_eq!(result, address!("90B1b09A9715CaDbFD9331b3A7652B24BfBEfD32"));
/// assert_eq!(
///     result,
///     compute_pool_address(
///         FACTORY_ADDRESS,
///         DAI_ADDRESS,
///         USDC_ADDRESS,
///         FeeAmount::LOW,
///         None,
///         None
///     )
/// );
/// ```
#[inline]
#[must_use]
pub fn compute_pool_address(
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    init_code_hash_manual_override: Option<B256>,
    chain_id: Option<alloy_primitives::ChainId>,
) -> Address {
    assert_ne!(token_a, token_b, "ADDRESSES");
    let (token_0, token_1) = if token_a < token_b {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };
    let fee: U24 = fee.into();
    let salt = keccak256((token_0, token_1, fee).abi_encode());
    const ZKSYNC_CHAIN_ID: u64 = ChainId::ZKSYNC as u64;

    // ZKSync uses a different create2 address computation
    // Most likely all ZKEVM chains will use the different computation from standard create2
    match chain_id {
        Some(ZKSYNC_CHAIN_ID) => compute_zksync_create2_address(
            factory,
            init_code_hash_manual_override.unwrap_or(b256!(
                "010013f177ea1fcbc4520f9a3ca7cd2d1d77959e05aa66484027cb38e712aeed"
            )),
            salt,
            None,
        ),
        _ => factory.create2(
            salt,
            init_code_hash_manual_override.unwrap_or(POOL_INIT_CODE_HASH),
        ),
    }
}
