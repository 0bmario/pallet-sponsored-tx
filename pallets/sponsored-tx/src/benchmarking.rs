//! Benchmarks for the sponsored transaction pallet.

use super::*;
use frame::{
	deps::{
		frame_benchmarking::v2::*,
		frame_support::traits::{fungible::Mutate, Get},
	},
	prelude::*,
};
use frame_system::RawOrigin;
use polkadot_sdk::sp_std::vec::Vec;

const SEED: u32 = 0;

fn minimum_balance<T: Config>() -> BalanceOf<T> {
	<T as pallet_balances::Config>::ExistentialDeposit::get().max(1u32.into())
}

fn default_initial_budget<T: Config>() -> BalanceOf<T> {
	// Benchmarks need held balances that stay valid for runtimes with large existential deposits.
	minimum_balance::<T>().saturating_mul(10u32.into())
}

fn default_budget_delta<T: Config>() -> BalanceOf<T> {
	minimum_balance::<T>().saturating_mul(2u32.into())
}

fn setup_balance<T: Config>() -> BalanceOf<T> {
	default_initial_budget::<T>().saturating_mul(10u32.into())
}

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let account = account(name, index, SEED);
	// Keep setup balances comfortably above the benchmark holds without overflowing issuance.
	pallet_balances::Pallet::<T>::set_balance(&account, setup_balance::<T>());
	account
}

fn policy_with_callers<T: Config>(count: u32, offset: u32) -> SponsorPolicy<T> {
	let callers: Vec<_> = (0..count).map(|i| account("allowed_caller", offset + i, SEED)).collect();

	SponsorPolicy {
		allowed_callers: callers
			.try_into()
			.expect("benchmark component never exceeds MaxAllowedCallers"),
		max_fee_per_tx: setup_balance::<T>(),
	}
}

fn register_sponsor_for_setup<T: Config>(
	sponsor: &T::AccountId,
	allowlist_len: u32,
	offset: u32,
) -> Result<BalanceOf<T>, BenchmarkError> {
	let initial_budget = default_initial_budget::<T>();
	Pallet::<T>::register_sponsor(
		RawOrigin::Signed(sponsor.clone()).into(),
		initial_budget,
		policy_with_callers::<T>(allowlist_len, offset),
	)
	.map_err(|_| BenchmarkError::Stop("benchmark sponsor setup failed"))?;

	Ok(initial_budget)
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn register_sponsor(
		n: Linear<1, { <T as Config>::MaxAllowedCallers::get() }>,
	) -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 0);
		let initial_budget = default_initial_budget::<T>();
		let policy = policy_with_callers::<T>(n, 0);

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), initial_budget, policy.clone());

		let state =
			Pallet::<T>::sponsor_state(&sponsor).ok_or(BenchmarkError::Stop("missing sponsor"))?;
		assert_eq!(state.policy, policy);
		assert!(state.active);
		assert_eq!(Pallet::<T>::budget_on_hold(&sponsor), initial_budget);
		Ok(())
	}

	#[benchmark]
	fn increase_budget() -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 1);
		let initial_budget = register_sponsor_for_setup::<T>(&sponsor, 1, 0)?;
		let amount = default_budget_delta::<T>();

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), amount);

		assert_eq!(Pallet::<T>::budget_on_hold(&sponsor), initial_budget + amount);
		Ok(())
	}

	#[benchmark]
	fn decrease_budget() -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 2);
		let initial_budget = register_sponsor_for_setup::<T>(&sponsor, 1, 0)?;
		let amount = default_budget_delta::<T>();

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), amount);

		assert_eq!(Pallet::<T>::budget_on_hold(&sponsor), initial_budget - amount);
		Ok(())
	}

	#[benchmark]
	fn set_policy(
		n: Linear<1, { <T as Config>::MaxAllowedCallers::get() }>,
	) -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 3);
		register_sponsor_for_setup::<T>(&sponsor, n, 0)?;
		let policy = policy_with_callers::<T>(n, 1_000);

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), policy.clone());

		let state =
			Pallet::<T>::sponsor_state(&sponsor).ok_or(BenchmarkError::Stop("missing sponsor"))?;
		assert_eq!(state.policy, policy);
		Ok(())
	}

	#[benchmark]
	fn pause() -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 4);
		register_sponsor_for_setup::<T>(&sponsor, 1, 0)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()));

		let state =
			Pallet::<T>::sponsor_state(&sponsor).ok_or(BenchmarkError::Stop("missing sponsor"))?;
		assert!(!state.active);
		Ok(())
	}

	#[benchmark]
	fn resume() -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 5);
		register_sponsor_for_setup::<T>(&sponsor, 1, 0)?;
		Pallet::<T>::pause(RawOrigin::Signed(sponsor.clone()).into())
			.map_err(|_| BenchmarkError::Stop("benchmark pause setup failed"))?;

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()));

		let state =
			Pallet::<T>::sponsor_state(&sponsor).ok_or(BenchmarkError::Stop("missing sponsor"))?;
		assert!(state.active);
		Ok(())
	}

	#[benchmark]
	fn unregister() -> Result<(), BenchmarkError> {
		let sponsor = funded_account::<T>("sponsor", 6);
		let initial_budget = register_sponsor_for_setup::<T>(&sponsor, 1, 0)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()));

		assert!(Pallet::<T>::sponsor_state(&sponsor).is_none());
		assert_eq!(Pallet::<T>::budget_on_hold(&sponsor), Zero::zero());
		assert_eq!(initial_budget, default_initial_budget::<T>());
		Ok(())
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use crate::pallet::Pallet as SponsoredTx;

		impl_benchmark_test_suite!(SponsoredTx, crate::mock::new_test_ext(), crate::mock::Test);
	}
}
