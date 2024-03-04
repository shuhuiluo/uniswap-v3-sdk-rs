use crate::constants::{FeeAmount, POOL_INIT_CODE_HASH};
use alloy_primitives::{keccak256, Address, B256};
use alloy_sol_types::SolValue;

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
///     )
/// );
/// ```
pub fn compute_pool_address(
    factory: Address,
    token_a: Address,
    token_b: Address,
    fee: FeeAmount,
    init_code_hash_manual_override: Option<B256>,
) -> Address {
    assert_ne!(token_a, token_b, "ADDRESSES");
    let (token_0, token_1) = if token_a < token_b {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };
    let pool_key = (token_0, token_1, fee as i32);
    factory.create2(
        keccak256(pool_key.abi_encode()),
        init_code_hash_manual_override.unwrap_or(POOL_INIT_CODE_HASH),
    )
}
