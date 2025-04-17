use crate::prelude::*;
use alloy_primitives::{aliases::U24, Bytes};
use alloy_sol_types::SolValue;
use uniswap_sdk_core::prelude::*;

#[inline]
fn encode_leg<'a, TP: TickDataProvider>(
    pool: &'a Pool<TP>,
    input_token: &'a Token,
) -> (&'a Token, Vec<u8>) {
    let output_token;
    let leg: (Address, U24) = if pool.token0.equals(input_token) {
        output_token = &pool.token1;
        (pool.token0.address(), pool.fee.into())
    } else {
        output_token = &pool.token0;
        (pool.token1.address(), pool.fee.into())
    };
    (output_token, leg.abi_encode_packed())
}

/// Converts a route to a hex encoded path.
///
/// ## Arguments
///
/// * `route`: the v3 path to convert to an encoded path
/// * `exact_output`: whether the route should be encoded in reverse, for making exact output swaps
#[inline]
pub fn encode_route_to_path<TInput, TOutput, TP>(
    route: &Route<TInput, TOutput, TP>,
    exact_output: bool,
) -> Bytes
where
    TInput: BaseCurrency,
    TOutput: BaseCurrency,
    TP: TickDataProvider,
{
    let mut path: Vec<u8> = Vec::with_capacity(23 * route.pools.len() + 20);
    if exact_output {
        let mut output_token = route.output.wrapped();
        for pool in route.pools.iter().rev() {
            let (input_token, leg) = encode_leg(pool, output_token);
            output_token = input_token;
            path.extend(leg);
        }
        path.extend(route.input.address().abi_encode_packed());
    } else {
        let mut input_token = route.input.wrapped();
        for pool in &route.pools {
            let (output_token, leg) = encode_leg(pool, input_token);
            input_token = output_token;
            path.extend(leg);
        }
        path.extend(route.output.address().abi_encode_packed());
    }
    path.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_route, tests::*};
    use alloy_primitives::hex;
    use once_cell::sync::Lazy;

    static POOL_1_2_LOW: Lazy<Pool> = Lazy::new(|| {
        Pool::new(
            TOKEN1.clone(),
            TOKEN2.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap()
    });

    static ROUTE_0_1_2: Lazy<Route<Token, Token, NoTickDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_1, POOL_1_2_LOW; TOKEN0, TOKEN2));

    static ROUTE_0_WETH: Lazy<Route<Token, Ether, NoTickDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_WETH, TOKEN0, ETHER));
    static ROUTE_0_1_WETH: Lazy<Route<Token, Ether, NoTickDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_1, POOL_1_WETH; TOKEN0, ETHER));
    static ROUTE_WETH_0_1: Lazy<Route<Ether, Token, NoTickDataProvider>> =
        Lazy::new(|| create_route!(POOL_0_WETH, POOL_0_1; ETHER, TOKEN1));

    #[test]
    fn pack_them_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1, false).to_vec(),
            hex!("0000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002")
        );
    }

    #[test]
    fn pack_them_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1, true).to_vec(),
            hex!("0000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn pack_them_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_2, false).to_vec(),
            hex!("0000000000000000000000000000000000000001000bb800000000000000000000000000000000000000020001f40000000000000000000000000000000000000003")
        );
    }

    #[test]
    fn pack_them_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_2, true).to_vec(),
            hex!("00000000000000000000000000000000000000030001f40000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_ETH_0, false).to_vec(),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_ETH_0, true).to_vec(),
            hex!("0000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0_1, false).to_vec(),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0_1, true).to_vec(),
            hex!("0000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_WETH, false).to_vec(),
            hex!("0000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_WETH, true).to_vec(),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_WETH, false).to_vec(),
            hex!("0000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_WETH, true).to_vec(),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }
}
