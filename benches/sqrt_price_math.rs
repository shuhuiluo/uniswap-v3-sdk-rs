use alloy_primitives::{keccak256, U256};
use alloy_sol_types::SolValue;
use criterion::{criterion_group, criterion_main, Criterion};
use uniswap_v3_math::sqrt_price_math;
use uniswap_v3_sdk::prelude::*;

fn pseudo_random(seed: u64) -> U256 {
    keccak256(seed.abi_encode()).into()
}

fn pseudo_random_128(seed: u64) -> u128 {
    let s: U256 = keccak256(seed.abi_encode()).into();
    u128::from_be_bytes(s.to_be_bytes::<32>()[..16].try_into().unwrap())
}

fn generate_inputs() -> Vec<(U256, u128, U256, bool)> {
    (0u64..100)
        .map(|i| {
            (
                pseudo_random(i),
                pseudo_random_128(i.pow(2)),
                pseudo_random(i.pow(3)),
                i % 2 == 0,
            )
        })
        .collect()
}

fn get_next_sqrt_price_from_input_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_next_sqrt_price_from_input", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &inputs {
                let _ = get_next_sqrt_price_from_input(*sqrt_price_x_96, *liquidity, *amount, *add);
            }
        })
    });
}

fn get_next_sqrt_price_from_input_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_next_sqrt_price_from_input_ref", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &inputs {
                let _ = sqrt_price_math::get_next_sqrt_price_from_input(
                    sqrt_price_x_96.to_ethers(),
                    *liquidity,
                    amount.to_ethers(),
                    *add,
                );
            }
        })
    });
}

fn get_next_sqrt_price_from_output_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_next_sqrt_price_from_output", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &inputs {
                let _ =
                    get_next_sqrt_price_from_output(*sqrt_price_x_96, *liquidity, *amount, *add);
            }
        });
    });
}

fn get_next_sqrt_price_from_output_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_next_sqrt_price_from_output_ref", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &inputs {
                let _ = sqrt_price_math::get_next_sqrt_price_from_output(
                    sqrt_price_x_96.to_ethers(),
                    *liquidity,
                    amount.to_ethers(),
                    *add,
                );
            }
        });
    });
}

fn get_amount_0_delta_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_0_delta", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in &inputs {
                let _ =
                    get_amount_0_delta(*sqrt_ratio_a_x96, *sqrt_ratio_b_x96, *liquidity, *round_up);
            }
        });
    });
}

fn get_amount_0_delta_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_0_delta_ref", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in &inputs {
                let _ = sqrt_price_math::_get_amount_0_delta(
                    sqrt_ratio_a_x96.to_ethers(),
                    sqrt_ratio_b_x96.to_ethers(),
                    *liquidity,
                    *round_up,
                );
            }
        });
    });
}

fn get_amount_1_delta_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_1_delta", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in &inputs {
                let _ =
                    get_amount_1_delta(*sqrt_ratio_a_x96, *sqrt_ratio_b_x96, *liquidity, *round_up);
            }
        });
    });
}

fn get_amount_1_delta_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_1_delta_ref", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, round_up) in &inputs {
                let _ = sqrt_price_math::_get_amount_1_delta(
                    sqrt_ratio_a_x96.to_ethers(),
                    sqrt_ratio_b_x96.to_ethers(),
                    *liquidity,
                    *round_up,
                );
            }
        });
    });
}

fn get_amount_0_delta_signed_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_0_delta_signed", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in &inputs {
                let _ = get_amount_0_delta_signed(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity as i128,
                );
            }
        });
    });
}

fn get_amount_0_delta_signed_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_0_delta_signed_ref", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in &inputs {
                let _ = sqrt_price_math::get_amount_0_delta(
                    sqrt_ratio_a_x96.to_ethers(),
                    sqrt_ratio_b_x96.to_ethers(),
                    *liquidity as i128,
                );
            }
        });
    });
}

fn get_amount_1_delta_signed_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_1_delta_signed", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in &inputs {
                let _ = get_amount_1_delta_signed(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity as i128,
                );
            }
        });
    });
}

fn get_amount_1_delta_signed_benchmark_ref(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_amount_1_delta_signed_ref", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, liquidity, sqrt_ratio_b_x96, _) in &inputs {
                let _ = sqrt_price_math::get_amount_1_delta(
                    sqrt_ratio_a_x96.to_ethers(),
                    sqrt_ratio_b_x96.to_ethers(),
                    *liquidity as i128,
                );
            }
        });
    });
}

criterion_group!(
    benches,
    get_next_sqrt_price_from_input_benchmark,
    get_next_sqrt_price_from_input_benchmark_ref,
    get_next_sqrt_price_from_output_benchmark,
    get_next_sqrt_price_from_output_benchmark_ref,
    get_amount_0_delta_benchmark,
    get_amount_0_delta_benchmark_ref,
    get_amount_1_delta_benchmark,
    get_amount_1_delta_benchmark_ref,
    get_amount_0_delta_signed_benchmark,
    get_amount_0_delta_signed_benchmark_ref,
    get_amount_1_delta_signed_benchmark,
    get_amount_1_delta_signed_benchmark_ref,
);
criterion_main!(benches);
