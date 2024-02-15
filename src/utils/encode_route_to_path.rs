use crate::prelude::Route;
use alloy_sol_types::{sol, SolType, SolValue};
use uniswap_sdk_core::prelude::*;

type TokenFee = sol!((address, uint24));

/// Converts a route to a hex encoded path
///
/// ## Arguments
///
/// * `route`: the v3 path to convert to an encoded path
/// * `exact_output`: whether the route should be encoded in reverse, for making exact output swaps
///
pub fn encode_route_to_path<TInput: CurrencyTrait, TOutput: CurrencyTrait, P>(
    route: &Route<TInput, TOutput, P>,
    exact_output: bool,
) -> Vec<u8> {
    let mut path: Vec<u8> = Vec::with_capacity(23 * route.pools.len() + 20);
    if exact_output {
        let mut output_token = &route.output.wrapped();
        for pool in route.pools.iter().rev() {
            let leg = if pool.token0.equals(output_token) {
                output_token = &pool.token1;
                (pool.token0.address(), pool.fee as u32)
            } else {
                output_token = &pool.token0;
                (pool.token1.address(), pool.fee as u32)
            };
            path.extend(TokenFee::abi_encode_packed(&leg));
        }
        path.extend(route.input.address().abi_encode_packed());
    } else {
        let mut input_token = &route.input.wrapped();
        for pool in route.pools.iter() {
            let leg = if pool.token0.equals(input_token) {
                input_token = &pool.token1;
                (pool.token0.address(), pool.fee as u32)
            } else {
                input_token = &pool.token0;
                (pool.token1.address(), pool.fee as u32)
            };
            path.extend(TokenFee::abi_encode_packed(&leg));
        }
        path.extend(route.output.address().abi_encode_packed());
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use alloy_primitives::hex;
    use once_cell::sync::Lazy;
    use uniswap_sdk_core::token;

    static ETHER: Lazy<Ether> = Lazy::new(|| Ether::on_chain(1));
    static TOKEN0: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x0000000000000000000000000000000000000001",
            18,
            "t0",
            "token0"
        )
    });
    static TOKEN1: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x0000000000000000000000000000000000000002",
            18,
            "t1",
            "token1"
        )
    });
    static TOKEN2: Lazy<Token> = Lazy::new(|| {
        token!(
            1,
            "0x0000000000000000000000000000000000000003",
            18,
            "t2",
            "token2"
        )
    });
    static WETH: Lazy<Token> = Lazy::new(|| ETHER.wrapped());

    static POOL_0_1_MEDIUM: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
        Pool::new(
            TOKEN0.clone(),
            TOKEN1.clone(),
            FeeAmount::MEDIUM,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap()
    });
    static POOL_1_2_LOW: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
        Pool::new(
            TOKEN1.clone(),
            TOKEN2.clone(),
            FeeAmount::LOW,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap()
    });
    static POOL_0_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
        Pool::new(
            TOKEN0.clone(),
            WETH.clone(),
            FeeAmount::MEDIUM,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap()
    });
    static POOL_1_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
        Pool::new(
            TOKEN1.clone(),
            WETH.clone(),
            FeeAmount::MEDIUM,
            encode_sqrt_ratio_x96(1, 1),
            0,
        )
        .unwrap()
    });

    static ROUTE_0_1: Lazy<Route<Token, Token, NoTickDataProvider>> = Lazy::new(|| {
        Route::new(
            vec![POOL_0_1_MEDIUM.clone()],
            TOKEN0.clone(),
            TOKEN1.clone(),
        )
    });
    static ROUTE_0_1_2: Lazy<Route<Token, Token, NoTickDataProvider>> = Lazy::new(|| {
        Route::new(
            vec![POOL_0_1_MEDIUM.clone(), POOL_1_2_LOW.clone()],
            TOKEN0.clone(),
            TOKEN2.clone(),
        )
    });

    static ROUTE_0_WETH: Lazy<Route<Token, Ether, NoTickDataProvider>> =
        Lazy::new(|| Route::new(vec![POOL_0_WETH.clone()], TOKEN0.clone(), ETHER.clone()));
    static ROUTE_0_1_WETH: Lazy<Route<Token, Ether, NoTickDataProvider>> = Lazy::new(|| {
        Route::new(
            vec![POOL_0_1_MEDIUM.clone(), POOL_1_WETH.clone()],
            TOKEN0.clone(),
            ETHER.clone(),
        )
    });
    static ROUTE_WETH_0: Lazy<Route<Ether, Token, NoTickDataProvider>> =
        Lazy::new(|| Route::new(vec![POOL_0_WETH.clone()], ETHER.clone(), TOKEN0.clone()));
    static ROUTE_WETH_0_1: Lazy<Route<Ether, Token, NoTickDataProvider>> = Lazy::new(|| {
        Route::new(
            vec![POOL_0_WETH.clone(), POOL_0_1_MEDIUM.clone()],
            ETHER.clone(),
            TOKEN1.clone(),
        )
    });

    #[test]
    fn pack_them_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1, false),
            hex!("0000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002")
        );
    }

    #[test]
    fn pack_them_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1, true),
            hex!("0000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn pack_them_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_2, false),
            hex!("0000000000000000000000000000000000000001000bb800000000000000000000000000000000000000020001f40000000000000000000000000000000000000003")
        );
    }

    #[test]
    fn pack_them_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_2, true),
            hex!("00000000000000000000000000000000000000030001f40000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0, false),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0, true),
            hex!("0000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0_1, false),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002")
        );
    }

    #[test]
    fn wrap_ether_input_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_WETH_0_1, true),
            hex!("0000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_input_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_WETH, false),
            hex!("0000000000000000000000000000000000000001000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_output_single_hop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_WETH, true),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000001")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_input_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_WETH, false),
            hex!("0000000000000000000000000000000000000001000bb80000000000000000000000000000000000000002000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
    }

    #[test]
    fn wrap_ether_output_for_exact_output_multihop() {
        assert_eq!(
            encode_route_to_path(&ROUTE_0_1_WETH, true),
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb80000000000000000000000000000000000000002000bb80000000000000000000000000000000000000001")
        );
    }
}
