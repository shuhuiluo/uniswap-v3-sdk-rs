use crate::prelude::{
    tick_math::{MAX_TICK, MIN_TICK},
    *,
};
use alloy_primitives::U160;
use dotenv::dotenv;
use once_cell::sync::Lazy;
use uniswap_sdk_core::{prelude::*, token};

pub(crate) static ETHER: Lazy<Ether> = Lazy::new(|| Ether::on_chain(1));
pub(crate) static TOKEN0: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000001",
        18,
        "t0",
        "token0"
    )
});
pub(crate) static TOKEN1: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000002",
        18,
        "t1",
        "token1"
    )
});
pub(crate) static TOKEN2: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000003",
        18,
        "t2",
        "token2"
    )
});
pub(crate) static TOKEN3: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000004",
        18,
        "t3",
        "token3"
    )
});
pub(crate) static WETH: Lazy<Token> = Lazy::new(|| ETHER.wrapped());
pub(crate) static USDC: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        6,
        "USDC",
        "USD Coin"
    )
});
pub(crate) static DAI: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "6B175474E89094C44Da98b954EedeAC495271d0F",
        18,
        "DAI",
        "DAI Stablecoin"
    )
});
pub(crate) const FEE_AMOUNT: FeeAmount = FeeAmount::MEDIUM;
pub(crate) const SQRT_RATIO_X96: U160 = U160::from_limbs([0, 4294967296, 0]);
pub(crate) const LIQUIDITY: u128 = 1_000_000;

pub(crate) static POOL_0_1: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN0.clone(),
        TOKEN1.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});
pub(crate) static POOL_0_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN0.clone(),
        WETH.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});
pub(crate) static POOL_1_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN1.clone(),
        WETH.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});

pub(crate) fn make_pool(token0: Token, token1: Token) -> Pool<TickListDataProvider> {
    Pool::new_with_tick_data_provider(
        token0,
        token1,
        FEE_AMOUNT,
        SQRT_RATIO_X96,
        LIQUIDITY,
        TickListDataProvider::new(
            vec![
                Tick::new(
                    nearest_usable_tick(MIN_TICK, FEE_AMOUNT.tick_spacing()).as_i32(),
                    LIQUIDITY,
                    LIQUIDITY as i128,
                ),
                Tick::new(
                    nearest_usable_tick(MAX_TICK, FEE_AMOUNT.tick_spacing()).as_i32(),
                    LIQUIDITY,
                    -(LIQUIDITY as i128),
                ),
            ],
            FEE_AMOUNT.tick_spacing().as_i32(),
        ),
    )
    .unwrap()
}

pub(crate) static RPC_URL: Lazy<String> = Lazy::new(|| {
    dotenv().ok();
    std::env::var("MAINNET_RPC_URL").unwrap().parse().unwrap()
});

// pub(crate) async fn make_provider() -> Provider<Http> {
//     Provider::<Http>::connect(&RPC_URL).await
// }
