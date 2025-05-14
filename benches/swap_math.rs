use alloy_primitives::{aliases::U24, keccak256, I256, U160, U256};
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

fn generate_inputs() -> Vec<(U160, U160, u128, I256, U24)> {
    (0u64..100)
        .map(|i| {
            (
                U160::saturating_from(pseudo_random(i)),
                U160::saturating_from(pseudo_random(i.pow(2))),
                pseudo_random_128(i.pow(3)),
                I256::from_raw(pseudo_random(i.pow(4))),
                U24::from(i),
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
    let inputs = generate_inputs()
        .into_iter()
        .map(|(a, b, c, d, e)| (U256::from(a), U256::from(b), c, d, e))
        .collect::<Vec<_>>();
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
                #[allow(clippy::missing_transmute_annotations)]
                let amount_remaining = unsafe { core::mem::transmute(*amount_remaining) };
                let _ = swap_math::compute_swap_step(
                    *sqrt_ratio_current_x96,
                    *sqrt_ratio_target_x96,
                    *liquidity,
                    amount_remaining,
                    fee_pips.into_limbs()[0] as u32,
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
