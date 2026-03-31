//! Minimal first-party client example for sponsored native fees.
//!
//! The runtime exposes a custom payment extension,
//! `SponsoredChargeTransactionPayment { tip, sponsor }`, so generic clients do not automatically
//! know how to encode the signed payload. This example shows the smallest working Subxt setup for:
//!
//! - registering a sponsor
//! - encoding the custom extension
//! - submitting a sponsored transaction from an allowlisted signer

use anyhow::Result;
use codec::{Compact, Encode};
use scale_decode::DecodeAsType;
use scale_encode::EncodeAsType;
use scale_info::PortableRegistry;
use subxt::{
	client::ClientState,
	config::{
		transaction_extensions::{self, Params},
		Config, DefaultExtrinsicParamsBuilder, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError,
	},
	dynamic::{self, Value},
	utils::{AccountId32, MultiAddress, MultiSignature},
	OnlineClient,
};
use subxt_signer::sr25519::dev;

// Type alias for the full tuple of extension parameters that Subxt needs
// when building a transaction for our custom config.
type SponsoredParams =
	<<SponsoredConfig as Config>::ExtrinsicParams as ExtrinsicParams<SponsoredConfig>>::Params;

// Empty enum used as a "tag" to tell Subxt how our runtime is configured.
// Subxt requires a concrete type that implements `Config` to know how to
// encode/decode transactions, addresses, and signatures for this specific chain.
pub enum SponsoredConfig {}

impl Config for SponsoredConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<Self::AccountId, ()>;
	type Signature = MultiSignature;
	type Hasher = subxt::config::substrate::BlakeTwo256;
	type Header = subxt::config::substrate::SubstrateHeader<u32, Self::Hasher>;
	// AnyOf lets Subxt pick only the extensions that the runtime actually uses.
	// Our runtime has the standard Substrate extensions plus our custom
	// SponsoredChargeTransactionPayment. Subxt matches them by name at runtime.
	type ExtrinsicParams = transaction_extensions::AnyOf<
		Self,
		(
			transaction_extensions::VerifySignature<Self>,
			transaction_extensions::CheckSpecVersion,
			transaction_extensions::CheckTxVersion,
			transaction_extensions::CheckNonce,
			transaction_extensions::CheckGenesis<Self>,
			transaction_extensions::CheckMortality<Self>,
			transaction_extensions::ChargeAssetTxPayment<Self>,
			SponsoredChargeTransactionPayment,
			transaction_extensions::CheckMetadataHash,
		),
	>;
	type AssetId = u32;
}

// Mirror of the runtime's `SponsoredChargeTransactionPayment` extension.
// Must match the on-chain SCALE encoding exactly: a compact-encoded tip
// followed by an optional sponsor AccountId.
#[derive(Clone, Debug, DecodeAsType, EncodeAsType)]
pub struct SponsoredChargeTransactionPayment {
	tip: Compact<u128>,
	sponsor: Option<AccountId32>,
}

// Subxt uses this impl to detect whether the runtime metadata contains our
// custom extension. It matches by name against the metadata's extension list.
impl<T: Config> transaction_extensions::TransactionExtension<T>
	for SponsoredChargeTransactionPayment
{
	type Decoded = Self;

	fn matches(identifier: &str, _type_id: u32, _types: &PortableRegistry) -> bool {
		identifier == "SponsoredChargeTransactionPayment"
	}
}

// Tells Subxt how to construct our extension from user-provided params.
impl<T: Config> ExtrinsicParams<T> for SponsoredChargeTransactionPayment {
	type Params = SponsoredChargeTransactionPaymentParams;

	fn new(_client: &ClientState<T>, params: Self::Params) -> Result<Self, ExtrinsicParamsError> {
		Ok(Self { tip: Compact(params.tip), sponsor: params.sponsor })
	}
}

// Tells Subxt how to serialize our extension into raw bytes for the
// signed transaction payload. Order matters: tip first, then sponsor.
impl ExtrinsicParamsEncoder for SponsoredChargeTransactionPayment {
	fn encode_value_to(&self, v: &mut Vec<u8>) {
		self.tip.encode_to(v);
		self.sponsor.encode_to(v);
	}
}

// User-facing params struct. Separates the "what the developer passes in"
// from the internal SCALE-encoded representation above.
#[derive(Clone, Debug, Default)]
pub struct SponsoredChargeTransactionPaymentParams {
	tip: u128,
	sponsor: Option<AccountId32>,
}

impl SponsoredChargeTransactionPaymentParams {
	pub fn new(tip: u128, sponsor: Option<AccountId32>) -> Self {
		Self { tip, sponsor }
	}
}

// Marker impl so Subxt accepts this as a valid extension parameter type.
impl<T: Config> Params<T> for SponsoredChargeTransactionPaymentParams {}

// Builder pattern for constructing the full set of extension params.
// Wraps `DefaultExtrinsicParamsBuilder` (which handles nonce, mortality, etc.)
// and adds our custom sponsor/tip fields on top.
#[derive(Default)]
pub struct SponsoredParamsBuilder {
	inner: DefaultExtrinsicParamsBuilder<SponsoredConfig>,
	tip: u128,
	sponsor: Option<AccountId32>,
}

impl SponsoredParamsBuilder {
	pub fn new() -> Self {
		Self::default()
	}

	// Set the sponsor account. The runtime will charge fees to this account
	// instead of the transaction signer.
	pub fn sponsor(mut self, sponsor: AccountId32) -> Self {
		self.sponsor = Some(sponsor);
		self
	}

	// Explicitly mark this transaction as unsponsored (normal fee payment).
	pub fn unsponsored(mut self) -> Self {
		self.sponsor = None;
		self
	}

	pub fn tip(mut self, tip: u128) -> Self {
		self.tip = tip;
		self.inner = self.inner.tip(tip);
		self
	}

	pub fn nonce(mut self, nonce: u64) -> Self {
		self.inner = self.inner.nonce(nonce);
		self
	}

	// Mortal transactions expire after N blocks. Prevents old signed
	// transactions from being replayed long after they were created.
	pub fn mortal(mut self, for_n_blocks: u64) -> Self {
		self.inner = self.inner.mortal(for_n_blocks);
		self
	}

	// Immortal transactions never expire. Use with caution.
	pub fn immortal(mut self) -> Self {
		self.inner = self.inner.immortal();
		self
	}

	// Assembles the final params tuple. We destructure the default builder's
	// output and swap in our custom extension params in place of the standard
	// ChargeTransactionPayment slot.
	pub fn build(self) -> SponsoredParams {
		let (
			verify_signature,
			check_spec_version,
			check_tx_version,
			check_nonce,
			check_genesis,
			check_mortality,
			charge_asset_tx_payment,
			_charge_tx_payment, // discarded: replaced by our sponsored extension
			check_metadata_hash,
		) = self.inner.build();

		(
			verify_signature,
			check_spec_version,
			check_tx_version,
			check_nonce,
			check_genesis,
			check_mortality,
			charge_asset_tx_payment,
			SponsoredChargeTransactionPaymentParams::new(self.tip, self.sponsor),
			check_metadata_hash,
		)
	}
}

// Builds the `SponsoredTx::register_sponsor` extrinsic using dynamic (untyped)
// encoding. Dynamic mode avoids needing a generated metadata module: we just
// pass pallet name + call name + arguments as runtime `Value`s.
fn register_sponsor_call(
	allowed_callers: &[AccountId32],
	initial_budget: u128,
	max_fee_per_tx: u128,
) -> impl subxt::tx::Payload {
	let policy = Value::named_composite([
		(
			"allowed_callers",
			Value::unnamed_composite(
				allowed_callers.iter().map(|account| Value::from_bytes(account.0)),
			),
		),
		("max_fee_per_tx", Value::u128(max_fee_per_tx)),
	]);

	dynamic::tx("SponsoredTx", "register_sponsor", vec![Value::u128(initial_budget), policy])
}

// A trivial `System::remark` call used to demonstrate a sponsored transaction.
fn sponsored_remark_call(remark: &[u8]) -> impl subxt::tx::Payload {
	dynamic::tx("System", "remark", vec![Value::from_bytes(remark)])
}

#[tokio::main]
async fn main() -> Result<()> {
	// Connect to a local dev node (or override via env var).
	let url =
		std::env::var("SPONSORED_TX_RPC_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	let client = OnlineClient::<SponsoredConfig>::from_url(url).await?;

	// Dev accounts: Alice will be the sponsor, Bob the sponsored user.
	let sponsor = dev::alice();
	let sponsored_user = dev::bob();
	let extra_allowed_user = dev::charlie();
	let sponsor_account = sponsor.public_key().to_account_id();
	let sponsored_user_account = sponsored_user.public_key().to_account_id();
	let extra_allowed_user_account = extra_allowed_user.public_key().to_account_id();

	// Step 1: Alice registers as a sponsor with a 2 DOT budget and 0.5 DOT max fee per tx.
	// This transaction is unsponsored (Alice pays her own fees for registering).
	let register_call = register_sponsor_call(
		&[sponsored_user_account.clone(), extra_allowed_user_account],
		2_000_000_000_000,
		500_000_000_000,
	);
	let register_params = SponsoredParamsBuilder::new().mortal(32).unsponsored().build();

	client
		.tx()
		.sign_and_submit_then_watch(&register_call, &sponsor, register_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	// Step 2: Bob submits a remark, but Alice pays the fees.
	// The `sponsor(sponsor_account)` call tells the runtime to charge Alice instead of Bob.
	let sponsored_remark = sponsored_remark_call(b"runtime native sponsored fees");
	let sponsored_params =
		SponsoredParamsBuilder::new().mortal(32).sponsor(sponsor_account).build();

	client
		.tx()
		.sign_and_submit_then_watch(&sponsored_remark, &sponsored_user, sponsored_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	println!("Registered Alice as sponsor and submitted a sponsored remark from Bob.");
	Ok(())
}
