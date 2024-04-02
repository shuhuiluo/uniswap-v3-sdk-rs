use super::abi::IPeripheryPaymentsWithFee;
use crate::utils::big_int_to_u256;
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolCall;
use uniswap_sdk_core::prelude::{FractionBase, Percent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeOptions {
    /// The percent of the output that will be taken as a fee.
    pub fee: Percent,
    /// The recipient of the fee.
    pub recipient: Address,
}

fn encode_fee_bips(fee: Percent) -> U256 {
    big_int_to_u256((fee * Percent::new(10000, 1)).quotient())
}

pub fn encode_unwrap_weth9(
    amount_minimum: U256,
    recipient: Address,
    fee_options: Option<FeeOptions>,
) -> Bytes {
    if let Some(fee_options) = fee_options {
        IPeripheryPaymentsWithFee::unwrapWETH9WithFeeCall {
            amountMinimum: amount_minimum,
            recipient,
            feeBips: encode_fee_bips(fee_options.fee),
            feeRecipient: fee_options.recipient,
        }
        .abi_encode()
    } else {
        IPeripheryPaymentsWithFee::unwrapWETH9Call {
            amountMinimum: amount_minimum,
            recipient,
        }
        .abi_encode()
    }
    .into()
}

pub fn encode_sweep_token(
    token: Address,
    amount_minimum: U256,
    recipient: Address,
    fee_options: Option<FeeOptions>,
) -> Bytes {
    if let Some(fee_options) = fee_options {
        IPeripheryPaymentsWithFee::sweepTokenWithFeeCall {
            token,
            amountMinimum: amount_minimum,
            recipient,
            feeBips: encode_fee_bips(fee_options.fee),
            feeRecipient: fee_options.recipient,
        }
        .abi_encode()
    } else {
        IPeripheryPaymentsWithFee::sweepTokenCall {
            token,
            amountMinimum: amount_minimum,
            recipient,
        }
        .abi_encode()
    }
    .into()
}

pub fn encode_refund_eth() -> Bytes {
    IPeripheryPaymentsWithFee::refundETHCall {}
        .abi_encode()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, hex, uint};
    use once_cell::sync::Lazy;

    const RECIPIENT: Address = address!("0000000000000000000000000000000000000003");
    const AMOUNT: U256 = uint!(123_U256);
    static FEE_OPTIONS: Lazy<FeeOptions> = Lazy::new(|| FeeOptions {
        fee: Percent::new(1, 1000),
        recipient: address!("0000000000000000000000000000000000000009"),
    });
    const TOKEN: Address = address!("0000000000000000000000000000000000000001");

    #[test]
    fn test_encode_unwrap_weth9_without_fee_options() {
        let calldata = encode_unwrap_weth9(AMOUNT, RECIPIENT, None);
        assert_eq!(
            calldata.to_vec(),
            hex!("49404b7c000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000003")
        );
    }

    #[test]
    fn test_encode_unwrap_weth9_with_fee_options() {
        let calldata = encode_unwrap_weth9(AMOUNT, RECIPIENT, Some(FEE_OPTIONS.clone()));
        assert_eq!(
            calldata.to_vec(),
            hex!("9b2c0a37000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000009")
        );
    }

    #[test]
    fn test_encode_sweep_token_without_fee_options() {
        let calldata = encode_sweep_token(TOKEN, AMOUNT, RECIPIENT, None);
        assert_eq!(
            calldata.to_vec(),
            hex!("df2ab5bb0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000003")
        );
    }

    #[test]
    fn test_encode_sweep_token_with_fee_options() {
        let calldata = encode_sweep_token(TOKEN, AMOUNT, RECIPIENT, Some(FEE_OPTIONS.clone()));
        assert_eq!(
            calldata.to_vec(),
            hex!("e0e189a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000007b0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000009")
        );
    }

    #[test]
    fn test_encode_refund_eth() {
        let calldata = encode_refund_eth();
        assert_eq!(calldata.to_vec(), hex!("12210e8a"));
    }
}
