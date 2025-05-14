use super::abi::ISelfPermit;
use alloy_primitives::{Bytes, Signature, B256, U256};
use alloy_sol_types::{eip712_domain, Eip712Domain, SolCall, SolStruct};
use uniswap_sdk_core::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ERC20PermitData<P: SolStruct> {
    pub domain: Eip712Domain,
    pub values: P,
}

impl<P: SolStruct> ERC20PermitData<P> {
    #[inline]
    #[must_use]
    pub fn eip712_signing_hash(&self) -> B256 {
        self.values.eip712_signing_hash(&self.domain)
    }
}

/// Get the EIP-2612 domain and values to sign for an ERC20 permit.
///
/// ## Arguments
///
/// * `permit`: The ERC20 permit
/// * `name`: The name of the contract
/// * `version`: The version of the contract
/// * `token`: The address of the token
/// * `chain_id`: The chain ID
///
/// ## Returns
///
/// The EIP-2612 domain and values to sign
///
/// ## Examples
///
/// ```
/// use alloy::signers::{local::PrivateKeySigner, SignerSync};
/// use alloy_primitives::{address, b256, uint, Signature, B256};
/// use alloy_sol_types::SolStruct;
/// use uniswap_v3_sdk::prelude::*;
///
/// let signer = PrivateKeySigner::random();
/// let permit = IERC20Permit::Permit {
///     owner: signer.address(),
///     spender: address!("0000000000000000000000000000000000000002"),
///     value: uint!(1_U256),
///     nonce: uint!(1_U256),
///     deadline: uint!(123_U256),
/// };
/// assert_eq!(
///     permit.eip712_type_hash(),
///     b256!("6e71edae12b1b97f4d1f60370fef10105fa2faae0126114a169c64845d6126c9")
/// );
/// assert_eq!(
///     IDaiPermit::Permit {
///         holder: signer.address(),
///         spender: address!("0000000000000000000000000000000000000002"),
///         nonce: uint!(1_U256),
///         expiry: uint!(123_U256),
///         allowed: true,
///     }
///     .eip712_type_hash(),
///     b256!("ea2aa0a1be11a07ed86d755c93467f4f82362b452371d1ba94d1715123511acb")
/// );
///
/// let permit_data = get_erc20_permit_data(
///     permit,
///     "ONE",
///     "1",
///     address!("0000000000000000000000000000000000000001"),
///     1,
/// );
///
/// // Derive the EIP-712 signing hash.
/// let hash: B256 = permit_data.eip712_signing_hash();
///
/// let signature: Signature = signer.sign_hash_sync(&hash).unwrap();
/// assert_eq!(
///     signature.recover_address_from_prehash(&hash).unwrap(),
///     signer.address()
/// );
/// ```
#[inline]
#[must_use]
pub fn get_erc20_permit_data<P: SolStruct>(
    permit: P,
    name: &'static str,
    version: &'static str,
    token: Address,
    chain_id: u64,
) -> ERC20PermitData<P> {
    let domain = eip712_domain! {
        name: name,
        version: version,
        chain_id: chain_id,
        verifying_contract: token,
    };
    ERC20PermitData {
        domain,
        values: permit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardPermitArguments {
    pub signature: Signature,
    pub amount: U256,
    pub deadline: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowedPermitArguments {
    pub signature: Signature,
    pub nonce: U256,
    pub expiry: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermitOptions {
    Standard(StandardPermitArguments),
    Allowed(AllowedPermitArguments),
}

impl StandardPermitArguments {
    #[inline]
    #[must_use]
    pub const fn new(r: U256, s: U256, v: bool, amount: U256, deadline: U256) -> Self {
        Self {
            signature: Signature::new(r, s, v),
            amount,
            deadline,
        }
    }
}

impl AllowedPermitArguments {
    #[inline]
    #[must_use]
    pub const fn new(r: U256, s: U256, v: bool, nonce: U256, expiry: U256) -> Self {
        Self {
            signature: Signature::new(r, s, v),
            nonce,
            expiry,
        }
    }
}

#[inline]
#[must_use]
pub fn encode_permit(token: &impl BaseCurrency, options: PermitOptions) -> Bytes {
    match options {
        PermitOptions::Standard(args) => ISelfPermit::selfPermitCall {
            token: token.address(),
            value: args.amount,
            deadline: args.deadline,
            v: args.signature.v() as u8 + 27,
            r: args.signature.r().into(),
            s: args.signature.s().into(),
        }
        .abi_encode(),
        PermitOptions::Allowed(args) => ISelfPermit::selfPermitAllowedCall {
            token: token.address(),
            nonce: args.nonce,
            expiry: args.expiry,
            v: args.signature.v() as u8 + 27,
            r: args.signature.r().into(),
            s: args.signature.s().into(),
        }
        .abi_encode(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{hex, uint};
    use once_cell::sync::Lazy;
    use uniswap_sdk_core::token;

    static TOKEN: Lazy<Token> =
        Lazy::new(|| token!(1, "0000000000000000000000000000000000000001", 18));

    #[test]
    fn test_encode_permit_standard() {
        let standard_permit_options = StandardPermitArguments::new(
            uint!(1_U256),
            uint!(2_U256),
            false,
            uint!(123_U256),
            uint!(123_U256),
        );
        let calldata = encode_permit(
            &TOKEN.clone(),
            PermitOptions::Standard(standard_permit_options),
        );
        assert_eq!(calldata, hex!("f3995c670000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000001b00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").to_vec());
    }

    #[test]
    fn test_encode_permit_allowed() {
        let allowed_permit_options = AllowedPermitArguments::new(
            uint!(1_U256),
            uint!(2_U256),
            false,
            uint!(123_U256),
            uint!(123_U256),
        );
        let calldata = encode_permit(
            &TOKEN.clone(),
            PermitOptions::Allowed(allowed_permit_options),
        );
        assert_eq!(calldata, hex!("4659a4940000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000001b00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").to_vec());
    }
}
