use alloy::uint;
use alloy_primitives::{keccak256, U160, U256};
use alloy_sol_types::SolValue;
use core::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use uniswap_v3_math::sqrt_price_math;
use uniswap_v3_sdk::prelude::*;

fn pseudo_random(seed: u64) -> U256 {
    keccak256(seed.abi_encode()).into()
}

fn pseudo_random_128(seed: u64) -> u128 {
    let s: U256 = keccak256(seed.abi_encode()).into();
    u128::from_be_bytes(s.to_be_bytes::<32>()[..16].try_into().unwrap())
}

fn generate_inputs() -> Vec<(U160, u128, U256, bool)> {
    let mut inputs = (0u64..100)
        .map(|i| {
            (
                U160::saturating_from(pseudo_random(i)),
                pseudo_random_128(i.pow(2)),
                pseudo_random(i.pow(3)),
                i % 2 == 0,
            )
        })
        .collect::<Vec<_>>();

    // Add edge cases
    inputs.extend([
        (U160::MIN, 1, uint!(1_U256), true),
        (U160::MAX, u128::MAX, U256::MAX, false),
        (uint!(1_U160) << 96, 1000000, uint!(1000_U256), true),
    ]);

    inputs
}

type SdkInputs = Vec<(U160, u128, U256, bool)>;
type RefInputs = Vec<(U256, u128, U256, bool)>;

fn generate_inputs_with_ref() -> (SdkInputs, RefInputs) {
    let sdk_inputs = generate_inputs();
    let ref_inputs = sdk_inputs
        .iter()
        .map(|(a, b, c, d)| (U256::from(*a), *b, *c, *d))
        .collect();
    (sdk_inputs, ref_inputs)
}

fn get_amount_inputs() -> Vec<(U160, U160, u128, bool)> {
    let mut inputs = (0u64..100)
        .map(|i| {
            (
                U160::saturating_from(pseudo_random(i)),
                U160::saturating_from(pseudo_random(i.pow(2))),
                pseudo_random_128(i.pow(3)),
                i % 2 == 0,
            )
        })
        .collect::<Vec<_>>();

    // Add edge cases
    inputs.extend([
        (U160::MIN, U160::MIN, 1, true),
        (U160::MAX, U160::MAX, u128::MAX, false),
        (uint!(1_U160) << 96, uint!(1_U160) << 96, 1000000, true),
    ]);

    inputs
}

type SdkAmountInputs = Vec<(U160, U160, u128, bool)>;
type RefAmountInputs = Vec<(U256, U256, u128, bool)>;

fn get_amount_inputs_with_ref() -> (SdkAmountInputs, RefAmountInputs) {
    let sdk_inputs = get_amount_inputs();
    let ref_inputs = sdk_inputs
        .iter()
        .map(|(a, b, c, d)| (U256::from(*a), U256::from(*b), *c, *d))
        .collect();
    (sdk_inputs, ref_inputs)
}

fn get_next_sqrt_price_from_input_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = generate_inputs_with_ref();
    let mut group = c.benchmark_group("get_next_sqrt_price_from_input");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &sdk_inputs {
                let _ = black_box(get_next_sqrt_price_from_input(
                    *sqrt_price_x_96,
                    *liquidity,
                    *amount,
                    *add,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &ref_inputs {
                let _ = black_box(sqrt_price_math::get_next_sqrt_price_from_input(
                    *sqrt_price_x_96,
                    *liquidity,
                    *amount,
                    *add,
                ));
            }
        })
    });

    group.finish();
}

fn get_next_sqrt_price_from_output_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = generate_inputs_with_ref();
    let mut group = c.benchmark_group("get_next_sqrt_price_from_output");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &sdk_inputs {
                let _ = black_box(get_next_sqrt_price_from_output(
                    *sqrt_price_x_96,
                    *liquidity,
                    *amount,
                    *add,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_price_x_96, liquidity, amount, add) in &ref_inputs {
                let _ = black_box(sqrt_price_math::get_next_sqrt_price_from_output(
                    *sqrt_price_x_96,
                    *liquidity,
                    *amount,
                    *add,
                ));
            }
        })
    });

    group.finish();
}

fn get_amount_0_delta_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = get_amount_inputs_with_ref();
    let mut group = c.benchmark_group("get_amount_0_delta");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up) in &sdk_inputs {
                let _ = black_box(get_amount_0_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity,
                    *round_up,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up) in &ref_inputs {
                let _ = black_box(sqrt_price_math::_get_amount_0_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity,
                    *round_up,
                ));
            }
        })
    });

    group.finish();
}

fn get_amount_1_delta_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = get_amount_inputs_with_ref();
    let mut group = c.benchmark_group("get_amount_1_delta");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up) in &sdk_inputs {
                let _ = black_box(get_amount_1_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity,
                    *round_up,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, round_up) in &ref_inputs {
                let _ = black_box(sqrt_price_math::_get_amount_1_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    *liquidity,
                    *round_up,
                ));
            }
        })
    });

    group.finish();
}

fn get_amount_0_delta_signed_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = get_amount_inputs_with_ref();
    let mut group = c.benchmark_group("get_amount_0_delta_signed");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, sign) in &sdk_inputs {
                let liquidity = if *sign {
                    *liquidity as i128
                } else {
                    -(*liquidity as i128)
                };
                let _ = black_box(get_amount_0_delta_signed(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    liquidity,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, sign) in &ref_inputs {
                let liquidity = if *sign {
                    *liquidity as i128
                } else {
                    -(*liquidity as i128)
                };
                let _ = black_box(sqrt_price_math::get_amount_0_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    liquidity,
                ));
            }
        })
    });

    group.finish();
}

fn get_amount_1_delta_signed_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = get_amount_inputs_with_ref();
    let mut group = c.benchmark_group("get_amount_1_delta_signed");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, sign) in &sdk_inputs {
                let liquidity = if *sign {
                    *liquidity as i128
                } else {
                    -(*liquidity as i128)
                };
                let _ = black_box(get_amount_1_delta_signed(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    liquidity,
                ));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for (sqrt_ratio_a_x96, sqrt_ratio_b_x96, liquidity, sign) in &ref_inputs {
                let liquidity = if *sign {
                    *liquidity as i128
                } else {
                    -(*liquidity as i128)
                };
                let _ = black_box(sqrt_price_math::get_amount_1_delta(
                    *sqrt_ratio_a_x96,
                    *sqrt_ratio_b_x96,
                    liquidity,
                ));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    get_next_sqrt_price_from_input_comparison,
    get_next_sqrt_price_from_output_comparison,
    get_amount_0_delta_comparison,
    get_amount_1_delta_comparison,
    get_amount_0_delta_signed_comparison,
    get_amount_1_delta_signed_comparison,
);
criterion_main!(benches);
