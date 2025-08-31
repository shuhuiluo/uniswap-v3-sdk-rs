use alloy_primitives::{uint, U256};
use core::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use uniswap_v3_math::bit_math;
use uniswap_v3_sdk::prelude::*;

const ONE: U256 = uint!(1_U256);

fn generate_test_values() -> Vec<U256> {
    let mut values = (0u8..=255).map(|i| ONE << i).collect::<Vec<_>>();
    // Add edge cases
    values.extend([ONE, U256::MAX]);
    values
}

fn most_significant_bit_comparison(c: &mut Criterion) {
    let values = generate_test_values();
    let mut group = c.benchmark_group("most_significant_bit");
    group.throughput(Throughput::Elements(values.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for value in &values {
                let _ = black_box(most_significant_bit(*value));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for value in &values {
                let _ = black_box(bit_math::most_significant_bit(*value));
            }
        })
    });

    group.finish();
}

fn least_significant_bit_comparison(c: &mut Criterion) {
    let values = generate_test_values();
    let mut group = c.benchmark_group("least_significant_bit");
    group.throughput(Throughput::Elements(values.len() as u64));

    group.bench_function("sdk", |b| {
        b.iter(|| {
            for value in &values {
                let _ = black_box(least_significant_bit(*value));
            }
        })
    });

    group.bench_function("reference", |b| {
        b.iter(|| {
            for value in &values {
                let _ = black_box(bit_math::least_significant_bit(*value));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    most_significant_bit_comparison,
    least_significant_bit_comparison
);
criterion_main!(benches);
