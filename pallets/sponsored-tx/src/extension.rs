//! Transaction extension for native sponsored fee payment.
//!
//! This module implements [`TransactionExtension`] via
//! [`SponsoredChargeTransactionPayment`]. The extension carries a `tip` and an optional
//! `sponsor: Option<AccountId>` in the signed payload.
//!
//! ## Lifecycle
//!
//! `TransactionExtension` defines a four-phase lifecycle for every extrinsic:
//!
//! 1. **`validate`** — Pool and block-import validation. Read-only checks that decide whether the
//!    transaction is acceptable. Returns a [`Val`](`SponsoredVal`) that captures everything the
//!    next phase needs (sponsor identity, worst-case fee, signer).
//!
//! 2. **`prepare`** — Runs once, immediately before dispatch, with mutable state access. Converts
//!    `Val` into [`Pre`](`SponsoredPre`) and performs any state changes that must be committed
//!    before the call executes (here: moving the estimated fee from the sponsor's budget hold into
//!    a per-transaction pending hold).
//!
//! 3. **dispatch** — The call itself. The extension is not involved.
//!
//! 4. **`post_dispatch_details`** — Runs after dispatch regardless of success/failure. Receives
//!    `Pre` and the actual execution results. Settles the real fee (slash from pending hold),
//!    restores any unused estimate back to budget hold, routes fee and tip credit, and returns the
//!    weight consumed by the settlement logic itself.
//!
//! ## Sponsored vs. Unsponsored
//!
//! When `sponsor = None`, the extension delegates entirely to
//! [`ChargeTransactionPayment`](`pallet_transaction_payment::ChargeTransactionPayment`) at
//! every phase — normal signer-pays-fee semantics are preserved unchanged.
//!
//! When `sponsor = Some(account)`, the extension runs its own sponsored path: validate checks
//! sponsor policy and budget, prepare escrows funds via the two-hold model (see crate-level
//! docs), and post_dispatch settles against the sponsor instead of the signer.

use crate::{pallet::Event, BalanceOf, Config, FeeCreditOf, Pallet};
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::marker::PhantomData;
use frame::prelude::*;
use polkadot_sdk::{
	frame_support::{
		dispatch::{DispatchInfo, DispatchResult, PostDispatchInfo},
		traits::{fungible::BalancedHold, Imbalance, OnUnbalanced},
	},
	pallet_balances, pallet_transaction_payment,
	sp_runtime::{
		traits::{
			AsSystemOriginSigner, DispatchInfoOf, Dispatchable, Implication, PostDispatchInfoOf,
			SaturatedConversion, Saturating, TransactionExtension, UniqueSaturatedInto, Zero,
		},
		transaction_validity::{
			InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
		},
	},
};

type FeeBalanceOf<T> = <<T as pallet_transaction_payment::Config>::OnChargeTransaction as
	pallet_transaction_payment::OnChargeTransaction<T>>::Balance;
// Keep the placeholder settlement accounting explicit until dedicated benchmarks replace it.
// Actual post-dispatch I/O: slash pending (2r, 2w) + restore_pending_to_budget: read pending
// (1r), release pending (2r, 2w), hold budget (2r, 2w) + deposit_event (0r, 1w).
const SPONSORED_POST_DISPATCH_READS: u64 = 7;
const SPONSORED_POST_DISPATCH_WRITES: u64 = 7;

#[repr(u8)]
enum InvalidSponsoredTransaction {
	UnsupportedOrigin = 0,
	UnknownSponsor = 1,
	InactiveSponsor = 2,
	CallerNotAllowed = 3,
	FeeCapExceeded = 4,
	InsufficientBudget = 5,
}

fn invalid(reason: InvalidSponsoredTransaction) -> TransactionValidityError {
	InvalidTransaction::Custom(reason as u8).into()
}

#[derive(
	Encode, Decode, DecodeWithMemTracking, TypeInfo, CloneNoBound, EqNoBound, PartialEqNoBound,
)]
#[scale_info(skip_type_params(T))]
/// Payment extension that optionally redirects transaction fees to an explicit sponsor.
pub struct SponsoredChargeTransactionPayment<T: Config> {
	#[codec(compact)]
	tip: BalanceOf<T>,
	sponsor: Option<T::AccountId>,
	_marker: PhantomData<T>,
}

impl<T: Config> SponsoredChargeTransactionPayment<T> {
	/// Create a new sponsored-payment extension with the given tip and optional sponsor.
	pub fn new(tip: BalanceOf<T>, sponsor: Option<T::AccountId>) -> Self {
		Self { tip, sponsor, _marker: PhantomData }
	}

	/// Return the encoded transaction tip.
	pub fn tip(&self) -> BalanceOf<T> {
		self.tip
	}

	/// Return the optional sponsor configured for this transaction.
	pub fn sponsor(&self) -> Option<&T::AccountId> {
		self.sponsor.as_ref()
	}
}

impl<T: Config> core::fmt::Debug for SponsoredChargeTransactionPayment<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(
			f,
			"SponsoredChargeTransactionPayment {{ tip: {:?}, sponsor: {:?} }}",
			self.tip, self.sponsor,
		)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		Ok(())
	}
}

#[derive(DebugNoBound)]
pub enum SponsoredVal<T: Config> {
	/// The normal unsponsored payment path handled by `ChargeTransactionPayment`.
	Unsponsored(pallet_transaction_payment::Val<T>),
	/// The sponsored path after validation has resolved the sponsor, signer, and worst-case fee.
	Sponsored {
		sponsor: T::AccountId,
		signer: T::AccountId,
		fee_with_tip: BalanceOf<T>,
		tip: BalanceOf<T>,
	},
}

/// Pre-dispatch state carried into post-dispatch settlement.
pub enum SponsoredPre<T: Config> {
	/// The normal unsponsored payment path handled by `ChargeTransactionPayment`.
	Unsponsored(pallet_transaction_payment::Pre<T>),
	/// Sponsored payment data needed to settle fees after dispatch.
	Sponsored {
		sponsor: T::AccountId,
		signer: T::AccountId,
		estimated_fee_with_tip: BalanceOf<T>,
		tip: BalanceOf<T>,
	},
}

impl<T: Config> core::fmt::Debug for SponsoredPre<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Unsponsored(pre) => write!(f, "Unsponsored({pre:?})"),
			Self::Sponsored { sponsor, signer, estimated_fee_with_tip, tip } => write!(
				f,
				"Sponsored {{ sponsor: {:?}, signer: {:?}, estimated_fee_with_tip: {:?}, tip: {:?} }}",
				sponsor,
				signer,
				estimated_fee_with_tip,
				tip,
			),
		}
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		Ok(())
	}
}

// The `weight()` function reports the extension's own overhead (validate + prepare I/O),
// NOT the weight of the dispatched call. For the sponsored path this is hand-counted from the
// storage accesses in validate and prepare; for the unsponsored path we delegate to the base
// `ChargeTransactionPayment` weight. Post-dispatch settlement weight is returned separately
// from `post_dispatch_details`.
impl<T> TransactionExtension<T::RuntimeCall> for SponsoredChargeTransactionPayment<T>
where
	T: Config + Send + Sync,
	T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	<T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + Clone,
	BalanceOf<T>: Send + Sync + UniqueSaturatedInto<FeeBalanceOf<T>>,
	FeeBalanceOf<T>: Send + Sync,
	FeeBalanceOf<T>: UniqueSaturatedInto<BalanceOf<T>>,
{
	const IDENTIFIER: &'static str = "SponsoredChargeTransactionPayment";
	type Implicit = ();
	/// Intermediate validation state. See module-level docs for the lifecycle overview.
	type Val = SponsoredVal<T>;
	/// Pre-dispatch state carried into post-dispatch settlement.
	type Pre = SponsoredPre<T>;

	fn weight(&self, call: &T::RuntimeCall) -> Weight {
		if self.sponsor.is_some() {
			// Sponsored validate+prepare: Sponsors read (1r) + budget_on_hold read (1r)
			// + release budget hold (2r, 2w) + hold pending (2r, 2w).
			T::DbWeight::get().reads_writes(6, 4)
		} else {
			pallet_transaction_payment::ChargeTransactionPayment::<T>::from(
				self.tip.saturated_into(),
			)
			.weight(call)
		}
	}

	fn validate(
		&self,
		origin: <T::RuntimeCall as Dispatchable>::RuntimeOrigin,
		call: &T::RuntimeCall,
		info: &DispatchInfoOf<T::RuntimeCall>,
		len: usize,
		_: (),
		_implication: &impl Implication,
		source: TransactionSource,
	) -> Result<
		(ValidTransaction, Self::Val, <T::RuntimeCall as Dispatchable>::RuntimeOrigin),
		TransactionValidityError,
	> {
		let base = pallet_transaction_payment::ChargeTransactionPayment::<T>::from(
			self.tip.saturated_into(),
		);
		let Some(sponsor) = self.sponsor.clone() else {
			// Preserve the runtime's normal payment semantics whenever no sponsor is supplied.
			let (validity, val, origin) =
				base.validate(origin, call, info, len, (), _implication, source)?;
			return Ok((validity, SponsoredVal::Unsponsored(val), origin));
		};

		// V1 only supports sponsor-paid fees for standard signed extrinsics. The runtime keeps
		// `AuthorizeCall` in the extension stack, so not every origin reaching this point is a
		// normal signer.
		let Some(signer) = origin.as_system_origin_signer().cloned() else {
			return Err(invalid(InvalidSponsoredTransaction::UnsupportedOrigin));
		};

		let Some(state) = Pallet::<T>::sponsor_state(&sponsor) else {
			return Err(invalid(InvalidSponsoredTransaction::UnknownSponsor));
		};
		if !state.active {
			return Err(invalid(InvalidSponsoredTransaction::InactiveSponsor));
		}
		if !state.policy.allowed_callers.iter().any(|account| account == &signer) {
			return Err(invalid(InvalidSponsoredTransaction::CallerNotAllowed));
		}

		let fee_with_tip: BalanceOf<T> = pallet_transaction_payment::Pallet::<T>::compute_fee(
			len as u32,
			info,
			self.tip.saturated_into(),
		)
		.saturated_into();
		if fee_with_tip > state.policy.max_fee_per_tx {
			return Err(invalid(InvalidSponsoredTransaction::FeeCapExceeded));
		}
		if Pallet::<T>::budget_on_hold(&sponsor) < fee_with_tip {
			return Err(invalid(InvalidSponsoredTransaction::InsufficientBudget));
		}

		let validity = ValidTransaction {
			priority: pallet_transaction_payment::ChargeTransactionPayment::<T>::get_priority(
				info,
				len,
				self.tip.saturated_into(),
				fee_with_tip.saturated_into(),
			),
			..Default::default()
		};

		Ok((
			validity,
			SponsoredVal::Sponsored { sponsor, signer, fee_with_tip, tip: self.tip },
			origin,
		))
	}

	fn prepare(
		self,
		val: Self::Val,
		origin: &<T::RuntimeCall as Dispatchable>::RuntimeOrigin,
		call: &T::RuntimeCall,
		info: &DispatchInfoOf<T::RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		match val {
			SponsoredVal::Unsponsored(val) => {
				let pre = pallet_transaction_payment::ChargeTransactionPayment::<T>::from(
					self.tip.saturated_into(),
				)
				.prepare(val, origin, call, info, len)?;
				Ok(SponsoredPre::Unsponsored(pre))
			},
			SponsoredVal::Sponsored { sponsor, signer, fee_with_tip, tip } => {
				// Reserve the worst-case fee before dispatch so post-dispatch settlement can only
				// consume funds already isolated for this transaction.
				Pallet::<T>::move_budget_to_pending(&sponsor, fee_with_tip)?;
				Ok(SponsoredPre::Sponsored {
					sponsor,
					signer,
					estimated_fee_with_tip: fee_with_tip,
					tip,
				})
			},
		}
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		info: &DispatchInfoOf<T::RuntimeCall>,
		post_info: &PostDispatchInfoOf<T::RuntimeCall>,
		len: usize,
		result: &DispatchResult,
	) -> Result<Weight, TransactionValidityError> {
		match pre {
			SponsoredPre::Unsponsored(pre) => {
				pallet_transaction_payment::ChargeTransactionPayment::<T>::post_dispatch_details(
					pre, info, post_info, len, result,
				)
			},
			SponsoredPre::Sponsored { sponsor, signer, estimated_fee_with_tip, tip } => {
				// Placeholder settlement weight until dedicated extension benchmarks land.
				// This covers the current path shape: slash pending hold, inspect/restore any
				// remainder, and deposit the settlement event.
				let settlement_weight = T::DbWeight::get()
					.reads_writes(SPONSORED_POST_DISPATCH_READS, SPONSORED_POST_DISPATCH_WRITES);
				let mut charged_fee_with_tip: BalanceOf<T> =
					pallet_transaction_payment::Pallet::<T>::compute_actual_fee(
						len as u32,
						info,
						post_info,
						tip.saturated_into(),
					)
					.saturated_into();
				// Validation and prepare reserve the worst-case fee. If post-dispatch accounting
				// produces something larger, clamp defensively rather than over-consuming funds
				// outside the reserved amount.
				if charged_fee_with_tip > estimated_fee_with_tip {
					log::error!(
							target: crate::LOG_TARGET,
						"actual sponsored fee exceeded estimate, clamping. estimated: {:?}, actual: {:?}",
						estimated_fee_with_tip,
						charged_fee_with_tip,
					);
					charged_fee_with_tip = estimated_fee_with_tip;
				}

				let pending_reason = Pallet::<T>::pending_hold_reason();
				let (credit, missing) = pallet_balances::Pallet::<T>::slash(
					&pending_reason,
					&sponsor,
					charged_fee_with_tip,
				);
				if !missing.is_zero() {
					log::error!(
						target: crate::LOG_TARGET,
						"sponsored pending hold for {:?} was short by {:?}",
						sponsor,
						missing,
					);
					charged_fee_with_tip = charged_fee_with_tip.saturating_sub(missing);
				}

				// Match the normal payment split by treating the tip as the first part of the
				// charged amount, then route fee and tip credit through the configured
				// destination hook.
				let actual_tip = tip.min(charged_fee_with_tip);
				let (tip_credit, fee_credit): (FeeCreditOf<T>, FeeCreditOf<T>) =
					credit.split(actual_tip);
				T::FeeDestination::on_unbalanceds(
					core::iter::once(fee_credit).chain(Some(tip_credit)),
				);

				// Whatever remains in the pending hold after slashing the actual fee becomes
				// available sponsor budget again.
				Pallet::<T>::restore_pending_to_budget(&sponsor);
				// Match `pallet_transaction_payment::TransactionFeePaid` convention: `actual_fee`
				// includes the tip so that `actual_fee = base_fee + tip`.
				Pallet::<T>::deposit_event(Event::SponsoredTransactionFeePaid {
					sponsor,
					signer,
					actual_fee: charged_fee_with_tip,
					tip: actual_tip,
				});

				Ok(settlement_weight)
			},
		}
	}
}
