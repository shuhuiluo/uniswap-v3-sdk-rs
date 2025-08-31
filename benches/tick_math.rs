use alloy_primitives::{aliases::I24, U160, U256};
use core::{hint::black_box, ops::Shl};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use uniswap_v3_math::tick_math;
use uniswap_v3_sdk::prelude::*;

fn generate_tick_inputs() -> Vec<I24> {
    let mut inputs = (-128..=128)
        .map(|i| I24::try_from(i).unwrap())
        .collect::<Vec<_>>();

    // Add edge cases
    inputs.extend([
        MIN_TICK,
        MAX_TICK,
        I24::ZERO,
        I24::try_from(1000).unwrap(),
        I24::try_from(-1000).unwrap(),
    ]);

    inputs
}

fn generate_sqrt_ratio_inputs() -> Vec<U160> {
    let mut inputs = (33u8..=159)
        .map(|i| U160::from(1).shl(i))
        .collect::<Vec<_>>();

    // Add edge cases
    inputs.extend([MIN_SQRT_RATIO, MAX_SQRT_RATIO, U160::from(Q96)]);

    inputs
}

fn generate_sqrt_ratio_inputs_with_ref() -> (Vec<U160>, Vec<U256>) {
    let sdk_inputs = generate_sqrt_ratio_inputs();
    let ref_inputs = sdk_inputs.iter().map(|&x| U256::from(x)).collect();
    (sdk_inputs, ref_inputs)
}

fn get_sqrt_ratio_at_tick_comparison(c: &mut Criterion) {
    let inputs = generate_tick_inputs();
    let mut group = c.benchmark_group("get_sqrt_ratio_at_tick");
    group.throughput(Throughput::Elements(inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for i in &inputs {
                let _ = black_box(get_sqrt_ratio_at_tick(*i));
            }
        })
    });

    // Reference uses i32 directly, so convert I24 to i32
    group.bench_function("reference", |b| {
        b.iter(|| {
            for i in &inputs {
                let _ = black_box(tick_math::get_sqrt_ratio_at_tick(i.as_i32()));
            }
        })
    });

    group.finish();
}

fn get_tick_at_sqrt_ratio_comparison(c: &mut Criterion) {
    let (sdk_inputs, ref_inputs) = generate_sqrt_ratio_inputs_with_ref();
    let mut group = c.benchmark_group("get_tick_at_sqrt_ratio");
    group.throughput(Throughput::Elements(sdk_inputs.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for sqrt_ratio in &sdk_inputs {
                let _ = black_box(get_tick_at_sqrt_ratio(*sqrt_ratio));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for sqrt_ratio in &ref_inputs {
                let _ = black_box(tick_math::get_tick_at_sqrt_ratio(*sqrt_ratio));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    get_sqrt_ratio_at_tick_comparison,
    get_tick_at_sqrt_ratio_comparison
);
criterion_main!(benches);
