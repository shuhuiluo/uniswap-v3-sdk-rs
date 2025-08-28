# Uniswap V3 SDK Examples

This directory contains practical examples demonstrating how to use the Uniswap V3 SDK for Rust to interact with Uniswap
V3 protocols.

## Prerequisites

- Rust 1.86 or later
- A mainnet RPC URL (for forking)

## Setup

1. Create a `.env` file in the project root:

```env
MAINNET_RPC_URL=https://your-ethereum-mainnet-rpc-url
```

2. Build the project with extensions feature:

```bash
cargo build --features extensions
```

## Examples

### Pool and Trading Examples

- **[from_pool_key_with_tick_data_provider.rs](./from_pool_key_with_tick_data_provider.rs)** - Demonstrates creating a
  pool with tick data provider and simulating swaps locally vs. using the quoter contract

- **[swap_router.rs](./swap_router.rs)** - Shows how to execute token swaps using the SwapRouter, including exact input
  and output swaps with proper slippage protection

### Position Management Examples

- **[nonfungible_position_manager.rs](./nonfungible_position_manager.rs)** - Demonstrates minting and managing liquidity
  positions as NFTs, including adding/removing liquidity and collecting fees

### Advanced Examples

- **[self_permit.rs](./self_permit.rs)** - Shows how to sign ERC20 permits for gasless approvals and encode `selfPermit`
  calls to the NonfungiblePositionManager

## Running Examples

Each example can be run independently:

```bash
# Run the pool creation and quoter comparison example
cargo run --example from_pool_key_with_tick_data_provider --features extensions

# Run the swap router example
cargo run --example swap_router --features extensions

# Run the position manager example
cargo run --example nonfungible_position_manager --features extensions

# Run the self permit example
cargo run --example self_permit --features extensions
```

## Key Concepts

### Tick Data Providers

The SDK supports multiple tick data provider implementations for fetching pool tick data:

- **SimpleTickDataProvider** - Fetches tick data directly via RPC calls
- **EphemeralTickDataProvider** - Fetches ticks using an ephemeral contract in a single `eth_call`
- **EphemeralTickMapDataProvider** - Fetches ticks and creates a `TickMap` for efficient access

### Core Components

- **Pool** - Represents a Uniswap V3 liquidity pool with price and liquidity state
- **Position** - Represents a liquidity position within a specific tick range
- **Route** - Defines a swap path through one or more pools
- **Trade** - Encapsulates swap execution details including slippage tolerance

### Transaction Building Process

1. **Create or load a pool** with the desired token pair and fee tier
2. **Calculate swap amounts** using local simulation or quoter contract
3. **Build transaction parameters** with appropriate slippage and deadline
4. **Execute transaction** through the appropriate Uniswap V3 contract

### Testing Setup

All examples use Anvil forking to create a local testnet that mirrors the mainnet state:

- Fork from mainnet block 17000000 for consistent results
- Create test accounts with ETH balances
- Set up token balances and approvals
- Execute transactions in the forked environment

## Common Patterns

- Use `uniswap_v3_sdk::prelude::*` for easy imports
- Set up providers using `setup_http_provider()` for read-only operations or `setup_anvil_fork_provider()` for
  transactions
- Handle both WETH and ERC20 tokens
- Use appropriate slippage tolerance (typically 0.5-1%) and deadline parameters

## Shared Utilities

The `common` module provides shared utilities across examples:

- **constants** - Chain IDs, contract addresses, and block numbers
- **providers** - HTTP and Anvil provider setup functions
- **tokens** - Pre-configured token instances (WETH, WBTC, USDC, etc.)

## Environment Variables

- **`MAINNET_RPC_URL`** (required) - Ethereum mainnet RPC endpoint for forking and data fetching

## Features

- **`extensions`** (required) - Enables RPC-based functionality for pool creation and tick data providers
- **`std`** - Standard library support (automatically enabled with extensions)

## Notes

- All examples use mainnet block 17000000 as a reference point for consistent results
- Token amounts are typically expressed in their smallest unit (e.g., wei for ETH)
- Always consider gas costs and slippage when executing real transactions
