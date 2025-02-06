# Uniswap V3 SDK Rust

[![Rust CI](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/shuhuiluo/uniswap-v3-sdk-rs?logo=rust&label=CodeRabbit&color=orange)
[![docs.rs](https://img.shields.io/docsrs/uniswap-v3-sdk)](https://docs.rs/uniswap-v3-sdk/latest)
[![crates.io](https://img.shields.io/crates/v/uniswap-v3-sdk.svg)](https://crates.io/crates/uniswap-v3-sdk)

A Rust SDK for building applications on top of Uniswap V3. Migration from the
TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).

It is feature-complete with unit tests matching the TypeScript SDK.

## Features

- Opinionated Rust implementation of the Uniswap V3 SDK with a focus on readability and performance
- Usage of [alloy-rs](https://github.com/alloy-rs) types
- Reimplementation of the math libraries in [Uniswap V3 Math In Rust](https://github.com/0xKitsune/uniswap-v3-math)
  based on optimizations presented in [Uni V3 Lib](https://github.com/Aperture-Finance/uni-v3-lib)
- Extensive unit tests and benchmarks
- An [`extensions`](./src/extensions) feature for additional functionalities related to Uniswap V3, including:

    - [`pool`](./src/extensions/pool.rs) module for creating a `Pool` struct from a pool key and fetching the
      liquidity map within a tick range for the specified pool, using RPC client
    - [`position`](./src/extensions/position.rs) module for creating a `Position` struct from a token id and fetching
      the state and pool for all positions of the specified owner, using RPC client, etc
    - [`price_tick_conversions`](./src/extensions/price_tick_conversions.rs) module for converting between prices and
      ticks
    - [`ephemeral_tick_data_provider`](./src/extensions/ephemeral_tick_data_provider.rs) module for fetching ticks using
      an [ephemeral contract](https://github.com/Aperture-Finance/Aperture-Lens/blob/904101e4daed59e02fd4b758b98b0749e70b583b/contracts/EphemeralGetPopulatedTicksInRange.sol)
      in a single `eth_call`
    - [`ephemeral_tick_map_data_provider`](./src/extensions/ephemeral_tick_map_data_provider.rs) fetches ticks in a
      single `eth_call` and creates a `TickMap`
    - [`tick_map`](./src/extensions/tick_map.rs) provides a way to access tick data directly from a hashmap, supposedly
      more efficient than `TickList`

<details>
  <summary>Expand to see the benchmarks</summary>

| Function               | Time      | Reference |
|------------------------|-----------|-----------|
| get_sqrt_ratio_at_tick | 4.0437 µs | 8.8094 µs |
| get_tick_at_sqrt_ratio | 21.232 µs | 31.547 µs |
| get_amount_0_delta     | 3.6099 µs | 4.4475 µs |
| get_amount_1_delta     | 2.5942 µs | 3.5725 µs |

</details>

## Getting started

Add the following to your `Cargo.toml` file:

```toml
uniswap-v3-sdk = { version = "4.0.0", features = ["extensions", "std"] }
```

### Usage

The package structure follows that of the TypeScript SDK, but with `snake_case` instead of `camelCase`.

For easy import, use the prelude:

```rust
use uniswap_v3_sdk::prelude::*;
```

## Note on `no_std`

By default, this library does not depend on the standard library (`std`). However, the `std` feature can be enabled.

## Examples

The code below shows an example of creating a pool with a tick map data provider and simulating a swap with it.

```rust,ignore
let pool = Pool::<EphemeralTickMapDataProvider>::from_pool_key_with_tick_data_provider(
    1,
    FACTORY_ADDRESS,
    wbtc.address(),
    weth.address(),
    FeeAmount::LOW,
    provider.clone(),
    block_id,
)
    .await
    .unwrap();
// Get the output amount from the pool
let amount_in = CurrencyAmount::from_raw_amount(wbtc.clone(), 100000000).unwrap();
let amount_out = pool.get_output_amount(&amount_in, None).unwrap();
```

For runnable examples, see the [examples](./examples) directory.

## Contributing

Contributions are welcome. Please open an issue if you have any questions or suggestions.

### Testing

Tests are run with

```shell
cargo test
```

for the core library. To run the tests for the extensions, use

```shell
cargo test --all-features --lib extensions -- --test-threads=1
```

To test a specific module, use `cargo test --test <module_name>`.

### Linting

Linting is done with `clippy` and `rustfmt`. To run the linter, use

```shell
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

### Benchmarking

Benchmarking is done with `criterion`. To run all the benchmarks, use

```shell
cargo bench
```

To run a specific benchmark, use `cargo bench --bench <bench_name>`.

## License

This project is licensed under the [MIT License](LICENSE).

## Acknowledgements

This project is inspired by and adapted from the following projects:

- [Uniswap V3 SDK](https://github.com/Uniswap/v3-sdk)
- [Uniswap SDK Core Rust](https://github.com/malik672/uniswap-sdk-core-rust)
- [Uniswap V3 Math In Rust](https://github.com/0xKitsune/uniswap-v3-math)
- [Uni V3 Lib](https://github.com/Aperture-Finance/uni-v3-lib)
- [Uniswap V3 Automation SDK](https://github.com/Aperture-Finance/uniswap-v3-automation-sdk)
