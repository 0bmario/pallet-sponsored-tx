//! Shared sponsor policy and state types.

use crate::{BalanceOf, Config};
use frame::prelude::*;

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebugNoBound,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
)]
#[scale_info(skip_type_params(T))]
/// Policy configured by a sponsor.
pub struct SponsorPolicy<T: Config> {
	/// Accounts allowed to submit transactions against this sponsor.
	pub allowed_callers: BoundedVec<T::AccountId, T::MaxAllowedCallers>,
	/// Maximum fee, including tip, that the sponsor is willing to pay per transaction.
	pub max_fee_per_tx: BalanceOf<T>,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebugNoBound,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
)]
#[scale_info(skip_type_params(T))]
/// Current on-chain state for a sponsor.
pub struct SponsorState<T: Config> {
	/// Whether the sponsor currently accepts sponsored transactions.
	pub active: bool,
	/// The sponsor policy applied during sponsored validation.
	pub policy: SponsorPolicy<T>,
}
