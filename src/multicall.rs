use crate::prelude::*;
use alloc::vec::Vec;
use alloy_primitives::Bytes;
use alloy_sol_types::{Error, SolCall};

#[inline]
#[must_use]
pub fn encode_multicall<B: Into<Bytes>>(data: Vec<B>) -> Bytes {
    if data.len() == 1 {
        data.into_iter().next().unwrap().into()
    } else {
        IMulticall::multicallCall {
            data: data.into_iter().map(Into::into).collect(),
        }
        .abi_encode()
        .into()
    }
}

#[inline]
pub fn decode_multicall<B, E>(encoded: E) -> Result<Vec<B>, Error>
where
    E: AsRef<[u8]>,
    B: From<Bytes>,
{
    IMulticall::multicallCall::abi_decode_validate(encoded.as_ref())
        .map(|decoded| decoded.data.into_iter().map(Into::into).collect())
}

pub trait Multicall: Sized {
    fn encode_multicall(self) -> Bytes;

    fn decode_multicall<E: AsRef<[u8]>>(encoded: E) -> Result<Self, Error>;
}

macro_rules! impl_multicall {
    ($($t:ty),*) => {
        $(
            impl Multicall for $t {
                #[inline]
                fn encode_multicall(self) -> Bytes {
                    encode_multicall(self)
                }

                #[inline]
                fn decode_multicall<E: AsRef<[u8]>>(encoded: E) -> Result<Self, Error> {
                    decode_multicall(encoded)
                }
            }
        )*
    };
}

impl_multicall!(Vec<Bytes>, Vec<Vec<u8>>);

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloy_primitives::hex;

    mod encode {
        use super::*;

        #[test]
        fn test_string_array_len_1() {
            let calldata = vec![vec![0x01]].encode_multicall();
            assert_eq!(calldata, vec![0x01]);
        }

        #[test]
        fn test_string_array_len_2() {
            let calldata = encode_multicall(vec![
                hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                hex!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
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
            let calldata_list = vec![
                hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                hex!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            ];
            let encoded = encode_multicall(calldata_list.clone());
            assert_eq!(
                encoded.to_vec(),
                hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000020aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000000000000000000000000000000000000000000020bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            );

            let decoded_calldata = <Vec<Vec<u8>>>::decode_multicall(encoded).unwrap();
            assert_eq!(decoded_calldata, calldata_list);
        }
    }
}
