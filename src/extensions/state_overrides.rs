//! ## State Overrides
//! This module provides functions to generate state overrides for ERC20 tokens.

use crate::prelude::Error;
use alloc::vec::Vec;
use alloy::{
    eips::eip2930::{AccessList, AccessListItem},
    network::{Network, TransactionBuilder},
    providers::Provider,
    rpc::types::state::{AccountOverride, StateOverride},
};
use alloy_primitives::{
    map::{B256HashMap, B256HashSet},
    Address, B256, U256,
};
use alloy_sol_types::SolCall;
use uniswap_lens::bindings::ierc20::IERC20;

#[inline]
pub async fn get_erc20_state_overrides<N, P>(
    token: Address,
    owner: Address,
    spender: Address,
    amount: U256,
    provider: &P,
) -> Result<StateOverride, Error>
where
    N: Network,
    P: Provider<N>,
{
    let balance_tx = N::TransactionRequest::default()
        .with_to(token)
        .with_gas_limit(0x11E1A300) // avoids "intrinsic gas too low" error
        .with_input(IERC20::balanceOfCall { account: owner }.abi_encode());
    let allowance_tx = N::TransactionRequest::default()
        .with_to(token)
        .with_gas_limit(0x11E1A300)
        .with_input(IERC20::allowanceCall { owner, spender }.abi_encode());
    let balance_access_list = provider.create_access_list(&balance_tx).await?.access_list;
    let allowance_access_list = provider
        .create_access_list(&allowance_tx)
        .await?
        .access_list;
    // tokens on L2 and those with a proxy will have more than one access list entry
    let filtered_balance_access_list = filter_access_list(balance_access_list, token);
    let filtered_allowance_access_list = filter_access_list(allowance_access_list, token);
    if filtered_balance_access_list.len() != 1 || filtered_allowance_access_list.len() != 1 {
        return Err(Error::InvalidAccessList);
    }
    // get rid of the storage key of implementation address
    let balance_slots_set = B256HashSet::from_iter(
        filtered_balance_access_list
            .into_iter()
            .next()
            .unwrap()
            .storage_keys,
    );
    let allowance_slots_set = B256HashSet::from_iter(
        filtered_allowance_access_list
            .into_iter()
            .next()
            .unwrap()
            .storage_keys,
    );
    let state_diff = B256HashMap::from_iter(
        balance_slots_set
            .symmetric_difference(&allowance_slots_set)
            .cloned()
            .map(|slot| (slot, B256::from(amount))),
    );
    if state_diff.len() != 2 {
        return Err(Error::InvalidAccessList);
    }
    Ok(StateOverride::from_iter([(
        token,
        AccountOverride {
            state_diff: Some(state_diff),
            ..Default::default()
        },
    )]))
}

fn filter_access_list(access_list: AccessList, token: Address) -> Vec<AccessListItem> {
    access_list
        .0
        .into_iter()
        .filter(|item| item.address == token)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::{address, U256};
    use uniswap_sdk_core::prelude::{BaseCurrency, NONFUNGIBLE_POSITION_MANAGER_ADDRESSES};

    #[tokio::test]
    async fn test_get_erc20_overrides() {
        let provider = PROVIDER.clone();
        let owner = address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640");
        let npm = *NONFUNGIBLE_POSITION_MANAGER_ADDRESSES.get(&1).unwrap();
        let amount = U256::from(1_000_000);
        let overrides = get_erc20_state_overrides(USDC.address(), owner, npm, amount, &provider)
            .await
            .unwrap();
        let usdc = IERC20::new(USDC.address(), provider);
        let balance = usdc
            .balanceOf(owner)
            .call()
            .overrides(overrides.clone())
            .await
            .unwrap();
        assert_eq!(balance, amount);
        let allowance = usdc
            .allowance(owner, npm)
            .call()
            .overrides(overrides)
            .await
            .unwrap();
        assert_eq!(allowance, amount);
    }
}
