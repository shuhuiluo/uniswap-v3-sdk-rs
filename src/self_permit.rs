use super::abi::ISelfPermit;
use alloy_primitives::{Bytes, Signature, U256};
use alloy_sol_types::SolCall;
use uniswap_sdk_core::prelude::{CurrencyTrait, Token};

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
    pub fn new(r: U256, s: U256, v: u64, amount: U256, deadline: U256) -> Self {
        Self {
            signature: Signature::from_rs_and_parity(r, s, v).unwrap(),
            amount,
            deadline,
        }
    }
}

impl AllowedPermitArguments {
    pub fn new(r: U256, s: U256, v: u64, nonce: U256, expiry: U256) -> Self {
        Self {
            signature: Signature::from_rs_and_parity(r, s, v).unwrap(),
            nonce,
            expiry,
        }
    }
}

pub fn encode_permit(token: Token, options: PermitOptions) -> Bytes {
    match options {
        PermitOptions::Standard(args) => ISelfPermit::selfPermitCall {
            token: token.address(),
            value: args.amount,
            deadline: args.deadline,
            v: args.signature.v().y_parity_byte(),
            r: args.signature.r().into(),
            s: args.signature.s().into(),
        }
        .abi_encode(),
        PermitOptions::Allowed(args) => ISelfPermit::selfPermitAllowedCall {
            token: token.address(),
            nonce: args.nonce,
            expiry: args.expiry,
            v: args.signature.v().y_parity_byte(),
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
    use alloy_primitives::{address, hex, uint};
    use once_cell::sync::Lazy;
    use uniswap_sdk_core::token;

    static TOKEN: Lazy<Token> =
        Lazy::new(|| token!(1, "0000000000000000000000000000000000000001", 18));

    #[test]
    fn test_encode_permit_standard() {
        let standard_permit_options = StandardPermitArguments::new(
            uint!(1_U256),
            uint!(2_U256),
            0,
            uint!(123_U256),
            uint!(123_U256),
        );
        let calldata = encode_permit(
            TOKEN.clone(),
            PermitOptions::Standard(standard_permit_options),
        );
        assert_eq!(calldata, hex!("f3995c670000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").to_vec());
    }

    #[test]
    fn test_encode_permit_allowed() {
        let allowed_permit_options = AllowedPermitArguments::new(
            uint!(1_U256),
            uint!(2_U256),
            0,
            uint!(123_U256),
            uint!(123_U256),
        );
        let calldata = encode_permit(
            TOKEN.clone(),
            PermitOptions::Allowed(allowed_permit_options),
        );
        assert_eq!(calldata, hex!("4659a4940000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000007b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").to_vec());
    }
}
