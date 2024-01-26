use super::abi::IMulticall;
use alloy_sol_types::SolCall;

pub fn encode_multicall(data: Vec<Vec<u8>>) -> Vec<u8> {
    if data.len() == 1 {
        data[0].clone()
    } else {
        IMulticall::multicallCall { data }.abi_encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    #[test]
    fn test_encode_multicall_string_array_len_1() {
        let calldata = encode_multicall(vec![vec![0x01]]);
        assert_eq!(calldata, vec![0x01]);
    }

    #[test]
    fn test_encode_multicall_string_array_len_2() {
        let calldata = encode_multicall(vec![
            hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").to_vec(),
            hex!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").to_vec(),
        ]);
        assert_eq!(
            calldata,
            hex!("ac9650d800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000020aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000000000000000000000000000000000000000000000000000020bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }
}
