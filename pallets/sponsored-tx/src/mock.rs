use crate::{SponsorPolicy, SponsoredChargeTransactionPayment};
use frame::{
	deps::frame_support::weights::{FixedFee, NoFee},
	prelude::*,
	runtime::prelude::*,
	testing_prelude::*,
};
use polkadot_sdk::{
	pallet_balances, pallet_transaction_payment,
	sp_runtime::{
		generic::{Block as GenericBlock, Header as GenericHeader, UncheckedExtrinsic},
		traits::{BlakeTwo256, Convert, IdentityLookup},
	},
};

pub type AccountId = u64;
pub type Balance = u64;
pub type Nonce = u64;
pub type Header = GenericHeader<u64, BlakeTwo256>;

#[frame_construct_runtime]
mod test_runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask,
		RuntimeViewFunction
	)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(2)]
	pub type TransactionPayment = pallet_transaction_payment;
	#[runtime::pallet_index(3)]
	pub type SponsoredTx = crate;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = Nonce;
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = GenericBlock<Header, UncheckedExtrinsic<AccountId, RuntimeCall, (), TxExtension>>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeHoldReason = RuntimeHoldReason;
	type AccountStore = System;
}

impl pallet_transaction_payment::Config for Test {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<1>;
	type WeightToFee = FixedFee<1, Balance>;
	type LengthToFee = NoFee<Balance>;
	type FeeMultiplierUpdate = ();
}

pub struct HoldReasonConverter;

impl Convert<crate::HoldReason, RuntimeHoldReason> for HoldReasonConverter {
	fn convert(reason: crate::HoldReason) -> RuntimeHoldReason {
		RuntimeHoldReason::SponsoredTx(reason)
	}
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type FeeDestination = ();
	type HoldReasonConverter = HoldReasonConverter;
	type MaxAllowedCallers = ConstU32<8>;
	type WeightInfo = ();
}

pub type TxExtension = (
	frame_system::CheckNonZeroSender<Test>,
	frame_system::CheckSpecVersion<Test>,
	frame_system::CheckTxVersion<Test>,
	frame_system::CheckGenesis<Test>,
	frame_system::CheckEra<Test>,
	frame_system::CheckNonce<Test>,
	frame_system::CheckWeight<Test>,
	SponsoredChargeTransactionPayment<Test>,
);

pub const EXT_VERSION: u8 = 0;

pub fn new_test_ext() -> TestState {
	let mut ext = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![(1, 1_000), (2, 1_000), (3, 1_000)],
		dev_accounts: None,
	}
	.assimilate_storage(&mut ext)
	.unwrap();
	let mut state: TestState = ext.into();
	state.execute_with(|| System::set_block_number(1));
	state
}

pub fn policy(callers: Vec<AccountId>, max_fee_per_tx: Balance) -> SponsorPolicy<Test> {
	SponsorPolicy {
		allowed_callers: callers.try_into().expect("callers fit in bounded vec"),
		max_fee_per_tx,
	}
}

pub fn remark_call() -> RuntimeCall {
	RuntimeCall::System(frame_system::Call::remark { remark: vec![1, 2, 3] })
}
