mod bit_math;
mod compute_pool_address;
mod encode_sqrt_ratio_x96;
mod full_math;
mod get_fee_growth_inside;
mod liquidity_math;
mod nearest_usable_tick;
mod position;
mod price_tick_conversions;
mod sqrt_price_math;
mod swap_math;
mod tick_list;
mod tick_math;

pub use bit_math::*;
pub use compute_pool_address::compute_pool_address;
pub use encode_sqrt_ratio_x96::encode_sqrt_ratio_x96;
pub use full_math::*;
pub use get_fee_growth_inside::*;
pub use liquidity_math::add_delta;
pub use nearest_usable_tick::nearest_usable_tick;
pub use position::get_tokens_owed;
pub use price_tick_conversions::*;
pub use sqrt_price_math::*;
pub use swap_math::compute_swap_step;
pub use tick_list::TickList;
pub use tick_math::*;

use alloy_primitives::U256;
use num_bigint::BigUint;

pub const Q96: U256 = U256::from_limbs([0, 4294967296, 0, 0]);
pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q192: U256 = U256::from_limbs([0, 0, 0, 1]);

pub fn u256_to_big_uint(x: U256) -> BigUint {
    BigUint::from_bytes_be(&x.to_be_bytes::<32>())
}
