# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Testing

```bash
# Run core library tests
cargo test

# Run extension tests (requires single thread)
cargo test --all-features --lib extensions -- --test-threads=1

# Run doc tests with all features
cargo test --doc --all-features

# Run a specific test module
cargo test --test <module_name>
```

### Linting

```bash
# Run clippy linter
cargo clippy --all-targets --all-features -- -D warnings

# Check formatting
cargo fmt --all -- --check
```

### Building

```bash
# Build core library
cargo build

# Build with std feature
cargo build --features std

# Build with extensions
cargo build --features extensions

# Build with all features
cargo build --all-features
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench <bench_name>
```

### Examples

```bash
# Run an example (requires extensions feature)
cargo run --example from_pool_key_with_tick_data_provider --features extensions
cargo run --example nonfungible_position_manager --features extensions
cargo run --example self_permit --features extensions
cargo run --example swap_router --features extensions
```

## Architecture

This is a Rust port of the TypeScript Uniswap V3 SDK with a focus on performance and `no_std` support.

### Core Structure

- **entities/** - Core domain models (Pool, Position, Route, Trade, Tick)
    - Uses trait-based tick data providers for flexible data sourcing
    - TickDataProvider trait enables different implementations (TickList, TickMap, etc.)

- **utils/** - Low-level math and utility functions
    - Optimized math implementations from Uniswap V3 Math and Uni V3 Lib
    - Critical functions: tick_math, sqrt_price_math, swap_math, liquidity calculations

- **extensions/** - Additional functionality requiring RPC provider
    - Pool creation from pool key with automatic tick data fetching
    - Position management with on-chain state queries
    - Multiple tick data provider implementations (Simple, Ephemeral, TickMap)
    - State overrides for simulation

### Key Components

- **nonfungible_position_manager.rs** - NFT position management operations
- **swap_router.rs** - Swap routing and encoding
- **quoter.rs** - Quote generation for swaps
- **multicall.rs** - Batched contract calls
- **payments.rs** - Payment helper functions
- **self_permit.rs** - Permit2 integration

### Dependencies

- **alloy-rs** - Ethereum types and provider interfaces
- **uniswap-sdk-core** - Core SDK types (Token, Currency, CurrencyAmount)
- **uniswap-lens** - Ephemeral contract interfaces for efficient data fetching

### Feature Flags

- `std` - Standard library support (disabled by default for no_std compatibility)
- `extensions` - RPC-based extensions for pool/position creation and tick data providers
- `parse_price` - Price parsing utilities (requires std)

### Testing Approach

Tests are embedded within modules using `#[cfg(test)]` blocks. Extension tests require single-threaded execution due to
shared state. Examples demonstrate real-world usage patterns with mainnet data.

### Environment Requirements

- MSRV: Rust 1.91
- Environment variable `MAINNET_RPC_URL` required for extension tests and examples
- Uses block 17000000 as reference point for consistent test results
