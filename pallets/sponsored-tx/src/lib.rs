//! # Sponsored Native Fees Pallet
//!
//! This pallet provides runtime-native sponsored fee payment for first-party clients.
//!
//! A sponsor registers a policy and escrows native balance on their own account using holds. An
//! approved signer can then submit a signed transaction that names the sponsor explicitly in the
//! custom payment extension. The runtime validates the sponsorship policy and charges the sponsor
//! instead of the signer.
//!
//! ## Pallet API
//!
//! The pallet exposes a small sponsor lifecycle API:
//!
//! - [`register_sponsor`](pallet::Pallet::register_sponsor)
//! - [`increase_budget`](pallet::Pallet::increase_budget)
//! - [`decrease_budget`](pallet::Pallet::decrease_budget)
//! - [`set_policy`](pallet::Pallet::set_policy)
//! - [`pause`](pallet::Pallet::pause)
//! - [`resume`](pallet::Pallet::resume)
//! - [`unregister`](pallet::Pallet::unregister)
//!
//! The stored sponsor policy is intentionally narrow for this first iteration, v1:
//!
//! - allowlisted callers
//! - a per-transaction fee cap
//! - an active or paused state
//!
//! ## Budget Model
//!
//! Sponsor funds are represented by native-token holds on the sponsor account rather than pallet
//! storage balances.
//!
//! Two hold reasons are used:
//!
//! - `SponsorshipBudget`: available sponsor capacity
//! - `SponsorshipPending`: the worst-case fee reserved for an in-flight sponsored transaction
//!
//! This keeps sponsor budget accounting aligned with the underlying balances pallet and makes the
//! available versus pending portions observable in standard chain state.
//!
//! ## Transaction Extension
//!
//! Sponsored fee payment is activated through [`SponsoredChargeTransactionPayment`]. The extension
//! carries both a `tip` and an explicit `sponsor: Option<AccountId>`.
//!
//! - `sponsor = None` preserves the normal unsponsored payment path.
//! - `sponsor = Some(account)` enables sponsor-paid native fees for standard signed extrinsics.
//!
//! The sponsored path validates policy and available budget up front, moves the estimated fee from
//! budget hold to pending hold during `prepare`, then settles the actual fee during
//! `post_dispatch`.
//!
//! ## Scope
//!
//! This pallet is intentionally scoped for deterministic, first-party-client sponsorship. It does
//! not implement sponsor discovery, rate limits, or generalized policy composition.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod extension;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
pub mod weights;

pub use extension::SponsoredChargeTransactionPayment;
pub use pallet::*;
pub use types::{SponsorPolicy, SponsorState};
pub use weights::WeightInfo;

use frame::prelude::*;
use polkadot_sdk::{
	frame_support::traits::{
		fungible::{InspectHold, MutateHold},
		OnUnbalanced,
	},
	pallet_balances, pallet_transaction_payment,
	sp_runtime::traits::Convert,
};

/// Log target used by sponsorship settlement and hold-management paths.
pub const LOG_TARGET: &str = "runtime::sponsored_tx";

/// Runtime balance type used by this pallet.
pub type BalanceOf<T> = <T as pallet_balances::Config>::Balance;
/// Imbalance type representing pending fee deductions that must be explicitly resolved.
pub type FeeCreditOf<T> = polkadot_sdk::frame_support::traits::fungible::Credit<
	<T as frame_system::Config>::AccountId,
	pallet_balances::Pallet<T>,
>;

#[frame::pallet]
pub mod pallet {
	use super::*;

	/// Hold reasons used to separate available sponsor budget from in-flight reservations.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Available sponsor budget held on the sponsor account.
		SponsorshipBudget,
		/// Worst-case fee reserved for an in-flight sponsored transaction.
		SponsorshipPending,
	}

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_balances::Config + pallet_transaction_payment::Config
	{
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Destination for fee and tip credit after sponsored settlement completes.
		type FeeDestination: OnUnbalanced<FeeCreditOf<Self>>;

		/// Converts pallet-local hold reasons into the runtime hold-reason type.
		type HoldReasonConverter: Convert<
			HoldReason,
			<Self as pallet_balances::Config>::RuntimeHoldReason,
		>;

		/// Maximum number of allowlisted callers per sponsor.
		#[pallet::constant]
		type MaxAllowedCallers: Get<u32>;

		/// Weight information for pallet dispatchables.
		type WeightInfo: crate::weights::WeightInfo;
	}

	// Empty wrapper struct for the pallet. FRAME attaches all storage,
	// calls, and hooks directly to this struct.
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Registered sponsors keyed by the account that ultimately pays sponsored fees.
	#[pallet::storage]
	pub type Sponsors<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, SponsorState<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A sponsor has been registered and the initial budget was placed on hold.
		SponsorRegistered { sponsor: T::AccountId, initial_budget: BalanceOf<T> },
		/// A sponsor increased their available held budget.
		BudgetIncreased { sponsor: T::AccountId, amount: BalanceOf<T>, new_budget: BalanceOf<T> },
		/// A sponsor decreased their available held budget.
		BudgetDecreased { sponsor: T::AccountId, amount: BalanceOf<T>, new_budget: BalanceOf<T> },
		/// A sponsor replaced their policy.
		SponsorPolicyUpdated { sponsor: T::AccountId },
		/// A sponsor has been paused and can no longer sponsor transactions.
		SponsorPaused { sponsor: T::AccountId },
		/// A sponsor has been resumed and may sponsor transactions again.
		SponsorResumed { sponsor: T::AccountId },
		/// A sponsor has been removed and any remaining budget released.
		SponsorUnregistered { sponsor: T::AccountId },
		/// A sponsored transaction settled fees against the sponsor instead of the signer.
		SponsoredTransactionFeePaid {
			sponsor: T::AccountId,
			signer: T::AccountId,
			actual_fee: BalanceOf<T>,
			tip: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is already registered as a sponsor.
		AlreadyRegistered,
		/// The caller is not registered as a sponsor.
		NotRegistered,
		/// The sponsor is already paused.
		AlreadyPaused,
		/// The sponsor is already active.
		AlreadyActive,
		/// A sponsor policy must contain at least one approved caller.
		EmptyAllowlist,
		/// A sponsor policy cannot contain duplicate approved callers.
		DuplicateAllowedCaller,
		/// Budget-changing operations require a non-zero amount.
		ZeroBudget,
		/// The available budget hold is smaller than the requested amount.
		InsufficientAvailableBudget,
		/// Pending funds must be fully settled before a sponsor can unregister.
		PendingBudgetNotEmpty,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register the caller as a sponsor and place the initial budget on hold.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::register_sponsor())]
		pub fn register_sponsor(
			origin: OriginFor<T>,
			initial_budget: BalanceOf<T>,
			policy: SponsorPolicy<T>,
		) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			ensure!(!initial_budget.is_zero(), Error::<T>::ZeroBudget);
			ensure!(!Sponsors::<T>::contains_key(&sponsor), Error::<T>::AlreadyRegistered);
			Self::ensure_valid_policy(&policy)?;

			pallet_balances::Pallet::<T>::hold(
				&Self::budget_hold_reason(),
				&sponsor,
				initial_budget,
			)?;
			Sponsors::<T>::insert(&sponsor, SponsorState { active: true, policy });

			Self::deposit_event(Event::SponsorRegistered { sponsor, initial_budget });
			Ok(())
		}

		/// Increase the caller's available sponsored-fee budget.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::increase_budget())]
		pub fn increase_budget(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroBudget);
			Self::ensure_registered(&sponsor)?;

			pallet_balances::Pallet::<T>::hold(&Self::budget_hold_reason(), &sponsor, amount)?;
			let new_budget = Self::budget_on_hold(&sponsor);
			Self::deposit_event(Event::BudgetIncreased { sponsor, amount, new_budget });
			Ok(())
		}

		/// Release part of the caller's available sponsored-fee budget.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::decrease_budget())]
		pub fn decrease_budget(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroBudget);
			Self::ensure_registered(&sponsor)?;
			ensure!(
				Self::budget_on_hold(&sponsor) >= amount,
				Error::<T>::InsufficientAvailableBudget
			);

			pallet_balances::Pallet::<T>::release(
				&Self::budget_hold_reason(),
				&sponsor,
				amount,
				polkadot_sdk::frame_support::traits::tokens::Precision::Exact,
			)?;
			let new_budget = Self::budget_on_hold(&sponsor);
			Self::deposit_event(Event::BudgetDecreased { sponsor, amount, new_budget });
			Ok(())
		}

		/// Replace the caller's sponsorship policy.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::set_policy())]
		pub fn set_policy(origin: OriginFor<T>, policy: SponsorPolicy<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			Self::ensure_valid_policy(&policy)?;
			Sponsors::<T>::try_mutate(&sponsor, |state| -> DispatchResult {
				let state = state.as_mut().ok_or(Error::<T>::NotRegistered)?;
				state.policy = policy;
				Ok(())
			})?;

			Self::deposit_event(Event::SponsorPolicyUpdated { sponsor });
			Ok(())
		}

		/// Pause the caller so future sponsored transactions fail validation.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::pause())]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			Sponsors::<T>::try_mutate(&sponsor, |state| -> DispatchResult {
				let state = state.as_mut().ok_or(Error::<T>::NotRegistered)?;
				ensure!(state.active, Error::<T>::AlreadyPaused);
				state.active = false;
				Ok(())
			})?;

			Self::deposit_event(Event::SponsorPaused { sponsor });
			Ok(())
		}

		/// Resume the caller so future sponsored transactions may validate again.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::resume())]
		pub fn resume(origin: OriginFor<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			Sponsors::<T>::try_mutate(&sponsor, |state| -> DispatchResult {
				let state = state.as_mut().ok_or(Error::<T>::NotRegistered)?;
				ensure!(!state.active, Error::<T>::AlreadyActive);
				state.active = true;
				Ok(())
			})?;

			Self::deposit_event(Event::SponsorResumed { sponsor });
			Ok(())
		}

		/// Remove the caller as a sponsor and release any remaining available budget.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as crate::Config>::WeightInfo::unregister())]
		pub fn unregister(origin: OriginFor<T>) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;
			Self::ensure_registered(&sponsor)?;
			ensure!(Self::pending_on_hold(&sponsor).is_zero(), Error::<T>::PendingBudgetNotEmpty);

			// Release the full budget hold so the sponsor recovers their funds.
			// `Exact` is deliberate: `budget` comes from `balance_on_hold` read above, so the
			// held amount must match. A mismatch would signal a broken invariant, in that case
			// we must fail rather than silently orphan held funds by removing the sponsor record.
			let budget = Self::budget_on_hold(&sponsor);
			if !budget.is_zero() {
				pallet_balances::Pallet::<T>::release(
					&Self::budget_hold_reason(),
					&sponsor,
					budget,
					polkadot_sdk::frame_support::traits::tokens::Precision::Exact,
				)?;
			}
			Sponsors::<T>::remove(&sponsor);
			Self::deposit_event(Event::SponsorUnregistered { sponsor });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn budget_hold_reason() -> <T as pallet_balances::Config>::RuntimeHoldReason {
			T::HoldReasonConverter::convert(HoldReason::SponsorshipBudget)
		}

		pub(crate) fn pending_hold_reason() -> <T as pallet_balances::Config>::RuntimeHoldReason {
			T::HoldReasonConverter::convert(HoldReason::SponsorshipPending)
		}

		pub(crate) fn budget_on_hold(who: &T::AccountId) -> BalanceOf<T> {
			pallet_balances::Pallet::<T>::balance_on_hold(&Self::budget_hold_reason(), who)
		}

		pub(crate) fn pending_on_hold(who: &T::AccountId) -> BalanceOf<T> {
			pallet_balances::Pallet::<T>::balance_on_hold(&Self::pending_hold_reason(), who)
		}

		pub(crate) fn sponsor_state(who: &T::AccountId) -> Option<SponsorState<T>> {
			Sponsors::<T>::get(who)
		}

		// `prepare` moves the worst-case fee out of the available budget into a dedicated pending
		// hold. This separates spendable sponsor capacity from the amount reserved for the current
		// transaction.
		pub(crate) fn move_budget_to_pending(
			who: &T::AccountId,
			amount: BalanceOf<T>,
		) -> Result<(), polkadot_sdk::sp_runtime::transaction_validity::TransactionValidityError>
		{
			use polkadot_sdk::{
				frame_support::traits::tokens::Precision,
				sp_runtime::{
					transaction_validity::{InvalidTransaction, TransactionValidityError},
					DispatchError,
				},
			};

			let to_validity =
				|_: DispatchError| TransactionValidityError::Invalid(InvalidTransaction::Payment);

			pallet_balances::Pallet::<T>::release(
				&Self::budget_hold_reason(),
				who,
				amount,
				Precision::Exact,
			)
			.map_err(to_validity)?;
			pallet_balances::Pallet::<T>::hold(&Self::pending_hold_reason(), who, amount)
				.map_err(to_validity)?;
			Ok(())
		}

		// After the actual fee has been slashed from the pending hold, any leftover estimate
		// becomes available sponsor budget again: pending hold → free → budget hold.
		pub(crate) fn restore_pending_to_budget(who: &T::AccountId) {
			use polkadot_sdk::frame_support::traits::tokens::Precision;

			let pending = Self::pending_on_hold(who);
			if pending.is_zero() {
				return;
			}

			// Step 1: release leftover from the pending hold (funds become free).
			let released = match pallet_balances::Pallet::<T>::release(
				&Self::pending_hold_reason(),
				who,
				pending,
				Precision::BestEffort,
			) {
				Ok(amount) => amount,
				Err(error) => {
					log::error!(
						target: LOG_TARGET,
						"failed to release sponsorship pending hold for {:?}: {:?}",
						who,
						error,
					);
					return;
				},
			};

			if released.is_zero() {
				return;
			}

			// Step 2: re-escrow the released funds under the budget hold.
			// In practice this should always succeed — we just freed exactly this amount.
			// If it fails (e.g. external freeze exceeds total holds), the funds stay
			// spendable rather than escrowed. Re-holding under pending_hold_reason is not
			// an option: it would create orphaned pending state and block unregistration.
			if let Err(error) =
				pallet_balances::Pallet::<T>::hold(&Self::budget_hold_reason(), who, released)
			{
				log::error!(
					target: LOG_TARGET,
					"failed to restore sponsorship budget hold for {:?}: {:?}",
					who,
					error,
				);
			}
		}

		fn ensure_registered(who: &T::AccountId) -> Result<(), Error<T>> {
			ensure!(Sponsors::<T>::contains_key(who), Error::<T>::NotRegistered);
			Ok(())
		}

		// The v1 policy is intentionally simple, but it still needs stable invariants for
		// validation weight and predictable semantics.
		fn ensure_valid_policy(policy: &SponsorPolicy<T>) -> Result<(), Error<T>> {
			ensure!(!policy.allowed_callers.is_empty(), Error::<T>::EmptyAllowlist);
			for (idx, caller) in policy.allowed_callers.iter().enumerate() {
				if policy.allowed_callers.iter().skip(idx + 1).any(|other| other == caller) {
					return Err(Error::<T>::DuplicateAllowedCaller);
				}
			}
			Ok(())
		}
	}
}
