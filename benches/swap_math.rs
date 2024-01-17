use alloy_primitives::{keccak256, I256, U256};
use alloy_sol_types::SolValue;
use criterion::{criterion_group, criterion_main, Criterion};
use uniswap_v3_math::swap_math;
use uniswap_v3_sdk::prelude::*;

fn pseudo_random(seed: u64) -> U256 {
    keccak256(seed.abi_encode()).into()
}

fn pseudo_random_128(seed: u64) -> u128 {
    let s: U256 = keccak256(seed.abi_encode()).into();
    u128::from_be_bytes(s.to_be_bytes::<32>()[..16].try_into().unwrap())
}

fn generate_inputs() -> Vec<(U256, U256, u128, I256, u32)> {
    (0u64..100)
        .map(|i| {
            (
                pseudo_random(i),
                pseudo_random(i.pow(2)),
                pseudo_random_128(i.pow(3)),
                I256::from_raw(pseudo_random(i.pow(4))),
                i as u32,
            )
        })
        .collect()
}

fn compute_swap_step_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("compute_swap_step", |b| {
        b.iter(|| {
            for (
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                amount_remaining,
                fee_pips,
            ) in &inputs
            {
                let _ = compute_swap_step(
                    *sqrt_ratio_current_x96,
                    *sqrt_ratio_target_x96,
                    *liquidity,
                    *amount_remaining,
                    *fee_pips,
                );
            }
        })
    });
}

fn compute_swap_step_benchmark_ref(c: &mut Criterion) {
    use ethers::types::{I256, U256};

    let inputs: Vec<(U256, U256, u128, I256, u32)> = generate_inputs()
        .into_iter()
        .map(|i| {
            (
                i.0.to_ethers(),
                i.1.to_ethers(),
                i.2,
                I256::from_raw(i.3.into_raw().to_ethers()),
                i.4,
            )
        })
        .collect();
    c.bench_function("compute_swap_step_ref", |b| {
        b.iter(|| {
            for (
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                amount_remaining,
                fee_pips,
            ) in &inputs
            {
                let _ = swap_math::compute_swap_step(
                    *sqrt_ratio_current_x96,
                    *sqrt_ratio_target_x96,
                    *liquidity,
                    *amount_remaining,
                    *fee_pips,
                );
            }
        })
    });
}

criterion_group!(
    benches,
    compute_swap_step_benchmark,
    compute_swap_step_benchmark_ref,
);
criterion_main!(benches);
