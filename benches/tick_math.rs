use alloy_primitives::{aliases::I24, U160, U256};
use core::ops::Shl;
use criterion::{criterion_group, criterion_main, Criterion};
use uniswap_v3_math::tick_math;
use uniswap_v3_sdk::prelude::*;

fn generate_inputs() -> Vec<I24> {
    (-128..=128).map(|i| I24::try_from(i).unwrap()).collect()
}

fn get_sqrt_ratio_at_tick_benchmark(c: &mut Criterion) {
    let inputs = generate_inputs();
    c.bench_function("get_sqrt_ratio_at_tick", |b| {
        b.iter(|| {
            for i in &inputs {
                let _ = get_sqrt_ratio_at_tick(*i);
            }
        })
    });
}

fn get_sqrt_ratio_at_tick_benchmark_ref(c: &mut Criterion) {
    c.bench_function("get_sqrt_ratio_at_tick_ref", |b| {
        b.iter(|| {
            for i in -128..=128 {
                let _ = tick_math::get_sqrt_ratio_at_tick(i);
            }
        })
    });
}

fn get_tick_at_sqrt_ratio_benchmark(c: &mut Criterion) {
    c.bench_function("get_tick_at_sqrt_ratio", |b| {
        b.iter(|| {
            for i in 33u8..=159 {
                let _ = get_tick_at_sqrt_ratio(U160::from(1).shl(i));
            }
        });
    });
}

fn get_tick_at_sqrt_ratio_benchmark_ref(c: &mut Criterion) {
    c.bench_function("get_tick_at_sqrt_ratio_ref", |b| {
        b.iter(|| {
            for i in 33u8..=159 {
                let _ = tick_math::get_tick_at_sqrt_ratio(U256::from(1).shl(i));
            }
        });
    });
}

criterion_group!(
    benches,
    get_sqrt_ratio_at_tick_benchmark,
    get_sqrt_ratio_at_tick_benchmark_ref,
    get_tick_at_sqrt_ratio_benchmark,
    get_tick_at_sqrt_ratio_benchmark_ref
);
criterion_main!(benches);
