# Uniswap V3 SDK Rust

[![Unit Tests](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/rust.yml)
[![Lint](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/lint.yml/badge.svg)](https://github.com/shuhuiluo/uniswap-v3-sdk-rs/actions/workflows/lint.yml)
[![crates.io](https://img.shields.io/crates/v/uniswap-v3-sdk.svg)](https://crates.io/crates/uniswap-v3-sdk)

A Rust SDK for building applications on top of Uniswap V3. Migration from the
TypeScript [Uniswap/v3-sdk](https://github.com/Uniswap/v3-sdk).

WIP.

## Features

- Opinionated Rust implementation of the Uniswap V3 SDK with a focus on readability and performance
- Usage of [alloy-rs](https://github.com/alloy-rs) types
- Reimplementation of the math libraries in [Uniswap V3 Math In Rust](https://github.com/0xKitsune/uniswap-v3-math)
  based on optimizations presented in [Uni V3 Lib](https://github.com/Aperture-Finance/uni-v3-lib)
- Extensive unit tests and benchmarks
- An `extensions` feature for additional functionality related to Uniswap V3

## Getting started

Add the following to your `Cargo.toml` file:

```toml
uniswap-v3-sdk = { version = "0.20.0", features = ["extensions"] }
```

### Usage

The package structure follows that of the TypeScript SDK, but with `snake_case` instead of `camelCase`.

For easy import, use the prelude:

```rust
use uniswap_v3_sdk::prelude::*;
```

## Contributing

Contributions are welcome. Please open an issue if you have any questions or suggestions.

### Testing

Tests are run with `cargo test`. To test a specific module, use `cargo test --test <module_name>`.

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
