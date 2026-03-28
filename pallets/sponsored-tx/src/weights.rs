//! Weights for `pallet-sponsored-tx`.
//!
//! These values are currently hand-written placeholders. They keep the pallet usable during the
//! first implementation pass, but should be replaced by benchmark-generated weights.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use core::marker::PhantomData;
use frame::{deps::frame_support::weights::constants::RocksDbWeight, prelude::*};

pub trait WeightInfo {
	fn register_sponsor() -> Weight;
	fn increase_budget() -> Weight;
	fn decrease_budget() -> Weight;
	fn set_policy() -> Weight;
	fn pause() -> Weight;
	fn resume() -> Weight;
	fn unregister() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn register_sponsor() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 2))
	}

	fn increase_budget() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 1))
	}

	fn decrease_budget() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 1))
	}

	fn set_policy() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}

	fn pause() -> Weight {
		Weight::from_parts(8_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}

	fn resume() -> Weight {
		Weight::from_parts(8_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
	}

	fn unregister() -> Weight {
		Weight::from_parts(16_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 2))
	}
}

impl WeightInfo for () {
	fn register_sponsor() -> Weight {
		Weight::from_parts(20_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
	}

	fn increase_budget() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 1))
	}

	fn decrease_budget() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 1))
	}

	fn set_policy() -> Weight {
		Weight::from_parts(12_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}

	fn pause() -> Weight {
		Weight::from_parts(8_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}

	fn resume() -> Weight {
		Weight::from_parts(8_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
	}

	fn unregister() -> Weight {
		Weight::from_parts(16_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
	}
}
