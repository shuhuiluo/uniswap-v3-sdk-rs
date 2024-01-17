use alloy_primitives::U256;
use criterion::{criterion_group, criterion_main, Criterion};
use std::ops::Shl;
use uniswap_v3_math::bit_math;
use uniswap_v3_sdk::prelude::*;

fn most_significant_bit_benchmark(c: &mut Criterion) {
    c.bench_function("most_significant_bit", |b| {
        b.iter(|| {
            for i in 1u8..=255 {
                let _ = most_significant_bit(U256::from(1).shl(i));
            }
        })
    });
}

fn most_significant_bit_benchmark_ref(c: &mut Criterion) {
    c.bench_function("most_significant_bit_ref", |b| {
        b.iter(|| {
            for i in 1u8..=255 {
                let _ = bit_math::most_significant_bit(U256::from(1).shl(i).to_ethers());
            }
        })
    });
}

fn least_significant_bit_benchmark(c: &mut Criterion) {
    c.bench_function("least_significant_bit", |b| {
        b.iter(|| {
            for i in 1u8..=255 {
                let _ = least_significant_bit(U256::from(1).shl(i));
            }
        });
    });
}

fn least_significant_bit_benchmark_ref(c: &mut Criterion) {
    c.bench_function("least_significant_bit_ref", |b| {
        b.iter(|| {
            for i in 1u8..=255 {
                let _ = bit_math::least_significant_bit(U256::from(1).shl(i).to_ethers());
            }
        });
    });
}

criterion_group!(
    benches,
    most_significant_bit_benchmark,
    most_significant_bit_benchmark_ref,
    least_significant_bit_benchmark,
    least_significant_bit_benchmark_ref
);
criterion_main!(benches);
