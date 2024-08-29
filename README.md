# Uniswap V3 SDK Rust

[![Unit Tests](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/uniswap-v3-sdk.svg)](https://crates.io/crates/uniswap-v3-sdk)

A Rust SDK for building applications on top of Uniswap V3. Migration from the
TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).

It is feature-complete with unit tests matching the TypeScript SDK. But error handling is not as graceful as one may
expect. The error handling is still a work in progress.

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

<details>
  <summary>Expand to see the benchmarks</summary>

| Function               | Time      | Reference |
|------------------------|-----------|-----------|
| most_significant_bit   | 8.3693 µs | 39.691 µs |
| least_significant_bit  | 5.0592 µs | 16.619 µs |
| get_sqrt_ratio_at_tick | 5.2105 µs | 71.137 µs |
| get_tick_at_sqrt_ratio | 34.331 µs | 191.08 µs |

</details>

## Getting started

Add the following to your `Cargo.toml` file:

```toml
uniswap-v3-sdk = { version = "0.32.0", features = ["extensions", "std"] }
```

### Usage

The package structure follows that of the TypeScript SDK, but with `snake_case` instead of `camelCase`.

For easy import, use the prelude:

```rust
use uniswap_v3_sdk::prelude::*;
```

## Note on `no_std`

By default, this library does not depend on the standard library (`std`). However, the `std` feature can be enabled to
use `thiserror` for error handling.

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

Linting is done with `clippy` and `rustfmt`. To run the linter, use:

```shell
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

### Benchmarking

Benchmarking is done with `criterion`. To run the benchmarks, use `cargo bench`.

## License

This project is licensed under the [MIT License](LICENSE).

## Acknowledgements

This project is inspired by and adapted from the following projects:

- [Uniswap V3 SDK](https://github.com/Uniswap/v3-sdk)
- [Uniswap SDK Core Rust](https://github.com/malik672/uniswap-sdk-core-rust)
- [Uniswap V3 Math In Rust](https://github.com/0xKitsune/uniswap-v3-math)
- [Uni V3 Lib](https://github.com/Aperture-Finance/uni-v3-lib)
- [uniswap-v3-automation-sdk](https://github.com/Aperture-Finance/uniswap-v3-automation-sdk)
