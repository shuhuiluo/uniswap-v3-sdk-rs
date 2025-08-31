use alloy_primitives::{aliases::U24, keccak256, uint, I256, U160, U256};
use alloy_sol_types::SolValue;
use core::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
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
    let mut inputs = (0u64..100)
        .map(|i| {
            (
                U160::saturating_from(pseudo_random(i)),
                U160::saturating_from(pseudo_random(i.pow(2))),
                pseudo_random_128(i.pow(3)),
                I256::from_raw(pseudo_random(i.pow(4))),
                U24::from(i),
            )
        })
        .collect::<Vec<_>>();

    // Add edge cases
    inputs.extend([
        (
            U160::MIN,
            U160::MIN,
            1,
            I256::from_raw(uint!(1_U256)),
            U24::from(500),
        ), // 0.05%
        (
            U160::MAX,
            U160::MAX,
            u128::MAX,
            I256::from_raw(U256::MAX - uint!(1_U256)),
            U24::from(3000),
        ), // 0.3%
        (
            U160::from(Q96),
            U160::from(Q96),
            1000000,
            I256::ZERO,
            U24::from(10000),
        ), // 1%
    ]);

    inputs
}

type SdkSwapInputs = Vec<(U160, U160, u128, I256, U24)>;
type RefSwapInputs = Vec<(U256, U256, u128, I256, u32)>;

fn generate_inputs_with_ref() -> (SdkSwapInputs, RefSwapInputs) {
    let sdk_inputs = generate_inputs();
    let ref_inputs = sdk_inputs
        .iter()
        .map(|(a, b, c, d, e)| {
            (
                U256::from(*a),
                U256::from(*b),
                *c,
                *d,
                e.into_limbs()[0] as u32,
            )
        })
        .collect();
    (sdk_inputs, ref_inputs)
}

fn compute_swap_step_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = generate_inputs_with_ref();
    let mut group = c.benchmark_group("compute_swap_step");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                amount_remaining,
                fee_pips,
            ) in &sdk_inputs
            {
                let _ = black_box(compute_swap_step(
                    *sqrt_ratio_current_x96,
                    *sqrt_ratio_target_x96,
                    *liquidity,
                    *amount_remaining,
                    *fee_pips,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (
                sqrt_ratio_current_x96,
                sqrt_ratio_target_x96,
                liquidity,
                amount_remaining,
                fee_pips,
            ) in &ref_inputs
            {
                let _ = black_box(swap_math::compute_swap_step(
                    *sqrt_ratio_current_x96,
                    *sqrt_ratio_target_x96,
                    *liquidity,
                    *amount_remaining,
                    *fee_pips,
                ));
            }
        })
    });

    group.finish();
}

criterion_group!(benches, compute_swap_step_comparison);
criterion_main!(benches);
