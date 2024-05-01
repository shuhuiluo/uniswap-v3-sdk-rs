use crate::prelude::*;
use alloy_primitives::U256;
use once_cell::sync::Lazy;
use uniswap_sdk_core::{prelude::*, token};

pub static ETHER: Lazy<Ether> = Lazy::new(|| Ether::on_chain(1));
pub static TOKEN0: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000001",
        18,
        "t0",
        "token0"
    )
});
pub static TOKEN1: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000002",
        18,
        "t1",
        "token1"
    )
});
pub static TOKEN2: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000003",
        18,
        "t2",
        "token2"
    )
});
pub static TOKEN3: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "0000000000000000000000000000000000000004",
        18,
        "t3",
        "token3"
    )
});
pub static WETH: Lazy<Token> = Lazy::new(|| Ether::on_chain(1).wrapped());
pub static USDC: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        6,
        "USDC",
        "USD Coin"
    )
});
pub static DAI: Lazy<Token> = Lazy::new(|| {
    token!(
        1,
        "6B175474E89094C44Da98b954EedeAC495271d0F",
        18,
        "DAI",
        "DAI Stablecoin"
    )
});
pub const FEE_AMOUNT: FeeAmount = FeeAmount::MEDIUM;
pub const SQRT_RATIO_X96: U256 = Q96;
pub const LIQUIDITY: u128 = 1_000_000;

pub static POOL_0_1: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN0.clone(),
        TOKEN1.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});
pub static POOL_0_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN0.clone(),
        WETH.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});
pub static POOL_1_WETH: Lazy<Pool<NoTickDataProvider>> = Lazy::new(|| {
    Pool::new(
        TOKEN1.clone(),
        WETH.clone(),
        FeeAmount::MEDIUM,
        encode_sqrt_ratio_x96(1, 1),
        0,
    )
    .unwrap()
});

pub fn make_pool(token0: Token, token1: Token) -> Pool<TickListDataProvider> {
    Pool::new_with_tick_data_provider(
        token0,
        token1,
        FEE_AMOUNT,
        SQRT_RATIO_X96,
        LIQUIDITY,
        TickListDataProvider::new(
            vec![
                Tick::new(
                    nearest_usable_tick(MIN_TICK, FEE_AMOUNT.tick_spacing()),
                    LIQUIDITY,
                    LIQUIDITY as i128,
                ),
                Tick::new(
                    nearest_usable_tick(MAX_TICK, FEE_AMOUNT.tick_spacing()),
                    LIQUIDITY,
                    -(LIQUIDITY as i128),
                ),
            ],
            FEE_AMOUNT.tick_spacing(),
        ),
    )
    .unwrap()
}
