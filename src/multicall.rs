use crate::prelude::*;
use alloy_primitives::Bytes;
use alloy_sol_types::{Error, SolCall};

#[inline]
#[must_use]
pub fn encode_multicall(data: Vec<Bytes>) -> Bytes {
    if data.len() == 1 {
        data[0].clone()
    } else {
        IMulticall::multicallCall { data }.abi_encode().into()
    }
}

#[inline]
pub fn decode_multicall(encoded: &Bytes) -> Result<Vec<Bytes>, Error> {
    IMulticall::multicallCall::abi_decode(encoded.as_ref(), true).map(|decoded| decoded.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    mod encode {
        use super::*;

        #[test]
        fn test_string_array_len_1() {
            let calldata = encode_multicall(vec![vec![0x01].into()]);
            assert_eq!(calldata, vec![0x01]);
        }

        #[test]
        fn test_string_array_len_2() {
            let calldata = encode_multicall(vec![
                hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").into(),
                hex!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").into(),
            ]);
            assert_eq!(
                calldata.to_vec(),
                hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000020aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000000000000000000000000000000000000000000020bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            );
        }
    }

    mod decode {
        use super::*;

        #[test]
        fn test_string_array_len_2() {
            let calldatas = vec![
                hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").into(),
                hex!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").into(),
            ];
            let multicall = encode_multicall(calldatas.clone());
            assert_eq!(
                multicall.to_vec(),
                hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000020aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000000000000000000000000000000000000000000020bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            );

            let decoded_calldata = decode_multicall(&multicall).unwrap();
            assert_eq!(decoded_calldata, calldatas);
        }
    }
}
