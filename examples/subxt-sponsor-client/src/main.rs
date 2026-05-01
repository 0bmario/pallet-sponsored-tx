//! Scripted first-party client demo for sponsored native fees.
//!
//! The runtime exposes a custom payment extension,
//! `SponsoredChargeTransactionPayment { tip, sponsor }`, so generic clients do not automatically
//! know how to encode the signed payload. This example shows a Subxt setup for:
//!
//! - registering a sponsor
//! - encoding the custom extension
//! - submitting a sponsored transaction from an allowlisted signer
//! - reading sponsor state, balance holds, and settlement events

use anyhow::{anyhow, Context, Result};
use codec::{Compact, Encode};
use scale_decode::DecodeAsType;
use scale_encode::EncodeAsType;
use scale_info::PortableRegistry;
use scale_value::{Composite, ValueDef};
use subxt::{
	client::ClientState,
	config::{
		transaction_extensions::{self, Params},
		Config, DefaultExtrinsicParamsBuilder, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError,
	},
	dynamic::{self, At, Value},
	utils::{AccountId32, MultiAddress, MultiSignature},
	OnlineClient,
};
use subxt_signer::sr25519::dev;

const UNIT: u128 = 1_000_000_000_000;
const INITIAL_BUDGET: u128 = 2 * UNIT;
const MAX_FEE_PER_TX: u128 = UNIT / 2;
const TIP: u128 = 0;

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

fn pause_call() -> impl subxt::tx::Payload {
	dynamic::tx("SponsoredTx", "pause", Vec::<Value>::new())
}

#[derive(Debug)]
struct DemoState {
	sponsor_active: Option<bool>,
	alice_free: u128,
	bob_free: u128,
	budget_hold: u128,
	pending_hold: u128,
}

#[derive(Debug)]
struct SponsoredFeePaid {
	actual_fee: u128,
	tip: u128,
}

async fn query_demo_state(
	client: &OnlineClient<SponsoredConfig>,
	sponsor: &AccountId32,
	sponsored_user: &AccountId32,
) -> Result<DemoState> {
	let storage = client.storage().at_latest().await?;

	let sponsor_query =
		dynamic::storage("SponsoredTx", "Sponsors", vec![Value::from_bytes(sponsor.0)]);
	let sponsor_active = storage
		.fetch(&sponsor_query)
		.await?
		.map(|value| value.to_value())
		.transpose()?
		.and_then(|value| value.at("active").and_then(Value::as_bool));

	let alice_free = account_free_balance(&storage, sponsor)
		.await
		.context("read Alice free balance")?;
	let bob_free = account_free_balance(&storage, sponsored_user)
		.await
		.context("read Bob free balance")?;
	let (budget_hold, pending_hold) =
		sponsor_holds(&storage, sponsor).await.context("read Alice balance holds")?;

	Ok(DemoState { sponsor_active, alice_free, bob_free, budget_hold, pending_hold })
}

async fn account_free_balance(
	storage: &subxt::storage::Storage<SponsoredConfig, OnlineClient<SponsoredConfig>>,
	account: &AccountId32,
) -> Result<u128> {
	let query = dynamic::storage("System", "Account", vec![Value::from_bytes(account.0)]);
	let value = storage.fetch(&query).await?.context("account storage missing")?.to_value()?;

	value
		.at("data")
		.at("free")
		.and_then(Value::as_u128)
		.context("free balance missing from System::Account")
}

async fn sponsor_holds(
	storage: &subxt::storage::Storage<SponsoredConfig, OnlineClient<SponsoredConfig>>,
	account: &AccountId32,
) -> Result<(u128, u128)> {
	let query = dynamic::storage("Balances", "Holds", vec![Value::from_bytes(account.0)]);
	let Some(value) = storage.fetch(&query).await? else {
		return Ok((0, 0));
	};
	let value = value.to_value()?;
	let mut budget = 0;
	let mut pending = 0;
	collect_sponsor_holds(&value, &mut budget, &mut pending);

	Ok((budget, pending))
}

fn collect_sponsor_holds(value: &Value<u32>, budget: &mut u128, pending: &mut u128) {
	if let Some(amount) = value.at("amount").and_then(Value::as_u128) {
		let reason = value.at("id").map(|id| format!("{id}")).unwrap_or_default();
		if reason.contains("SponsorshipBudget") {
			*budget = amount;
		} else if reason.contains("SponsorshipPending") {
			*pending = amount;
		}
	}

	match &value.value {
		ValueDef::Composite(Composite::Named(values)) => {
			for (_, child) in values {
				collect_sponsor_holds(child, budget, pending);
			}
		},
		ValueDef::Composite(Composite::Unnamed(values)) => {
			for child in values {
				collect_sponsor_holds(child, budget, pending);
			}
		},
		ValueDef::Variant(variant) => match &variant.values {
			Composite::Named(values) => {
				for (_, child) in values {
					collect_sponsor_holds(child, budget, pending);
				}
			},
			Composite::Unnamed(values) => {
				for child in values {
					collect_sponsor_holds(child, budget, pending);
				}
			},
		},
		ValueDef::BitSequence(_) | ValueDef::Primitive(_) => {},
	}
}

fn print_state(label: &str, state: &DemoState) {
	println!("{label}");
	println!("  sponsor active: {}", format_active(state.sponsor_active));
	println!("  Alice free: {}", format_balance(state.alice_free));
	println!("  Bob free: {}", format_balance(state.bob_free));
	println!("  Alice SponsorshipBudget hold: {}", format_balance(state.budget_hold));
	println!("  Alice SponsorshipPending hold: {}", format_balance(state.pending_hold));
}

fn format_active(active: Option<bool>) -> &'static str {
	match active {
		Some(true) => "yes",
		Some(false) => "paused",
		None => "not registered",
	}
}

fn format_balance(planck: u128) -> String {
	let whole = planck / UNIT;
	let frac = (planck % UNIT) / 100_000_000;
	format!("{whole}.{frac:04} UNIT")
}

fn find_sponsored_fee_paid(
	events: &subxt::blocks::ExtrinsicEvents<SponsoredConfig>,
) -> Result<SponsoredFeePaid> {
	for event in events.iter() {
		let event = event?;
		if event.pallet_name() == "SponsoredTx"
			&& event.variant_name() == "SponsoredTransactionFeePaid"
		{
			let fields = event.field_values()?;
			let actual_fee = fields
				.at("actual_fee")
				.and_then(Value::as_u128)
				.context("actual_fee missing from SponsoredTransactionFeePaid")?;
			let tip = fields
				.at("tip")
				.and_then(Value::as_u128)
				.context("tip missing from SponsoredTransactionFeePaid")?;
			return Ok(SponsoredFeePaid { actual_fee, tip });
		}
	}

	Err(anyhow!("SponsoredTransactionFeePaid event not found"))
}

#[tokio::main]
async fn main() -> Result<()> {
	// Connect to a local dev node (or override via env var).
	let url =
		std::env::var("SPONSORED_TX_RPC_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());
	println!("1. Connecting to {url}");
	let client = OnlineClient::<SponsoredConfig>::from_url(url).await?;

	// Dev accounts: Alice will be the sponsor, Bob the sponsored user.
	let sponsor = dev::alice();
	let sponsored_user = dev::bob();
	let extra_allowed_user = dev::charlie();
	let sponsor_account = sponsor.public_key().to_account_id();
	let sponsored_user_account = sponsored_user.public_key().to_account_id();
	let extra_allowed_user_account = extra_allowed_user.public_key().to_account_id();

	let state = query_demo_state(&client, &sponsor_account, &sponsored_user_account).await?;
	print_state("Initial state", &state);

	// Step 2: Alice registers as a sponsor with a 2 UNIT budget and 0.5 UNIT max fee per tx.
	// This transaction is unsponsored (Alice pays her own fees for registering).
	println!("\n2. Registering Alice as sponsor (unsponsored tx)");
	let register_call = register_sponsor_call(
		&[sponsored_user_account.clone(), extra_allowed_user_account],
		INITIAL_BUDGET,
		MAX_FEE_PER_TX,
	);
	let register_params = SponsoredParamsBuilder::new().mortal(32).unsponsored().build();

	client
		.tx()
		.sign_and_submit_then_watch(&register_call, &sponsor, register_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	let state = query_demo_state(&client, &sponsor_account, &sponsored_user_account).await?;
	print_state("After registration", &state);

	// Step 3: Bob submits a remark, but Alice pays the fees.
	// The `sponsor(sponsor_account)` call tells the runtime to charge Alice instead of Bob.
	println!("\n3. Bob submits sponsored System::remark; Alice pays");
	let sponsored_remark = sponsored_remark_call(b"runtime native sponsored fees");
	let sponsored_params = SponsoredParamsBuilder::new()
		.mortal(32)
		.tip(TIP)
		.sponsor(sponsor_account.clone())
		.build();

	let events = client
		.tx()
		.sign_and_submit_then_watch(&sponsored_remark, &sponsored_user, sponsored_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	let paid = find_sponsored_fee_paid(&events)?;
	println!(
		"  Event: SponsoredTransactionFeePaid actual_fee={} tip={}",
		format_balance(paid.actual_fee),
		format_balance(paid.tip)
	);

	let state = query_demo_state(&client, &sponsor_account, &sponsored_user_account).await?;
	print_state("After sponsored remark", &state);

	// Step 4: Pause Alice and show validation rejects a new sponsored transaction.
	println!("\n4. Pausing Alice sponsor");
	let pause_params = SponsoredParamsBuilder::new().mortal(32).unsponsored().build();
	client
		.tx()
		.sign_and_submit_then_watch(&pause_call(), &sponsor, pause_params)
		.await?
		.wait_for_finalized_success()
		.await?;

	let state = query_demo_state(&client, &sponsor_account, &sponsored_user_account).await?;
	print_state("After pause", &state);

	println!("\n5. Bob tries another sponsored remark while Alice is paused");
	let rejected_params = SponsoredParamsBuilder::new()
		.mortal(32)
		.sponsor(sponsor_account.clone())
		.build();
	let rejected = client
		.tx()
		.sign_and_submit_then_watch(
			&sponsored_remark_call(b"should be rejected"),
			&sponsored_user,
			rejected_params,
		)
		.await;
	match rejected {
		Ok(progress) => match progress.wait_for_finalized_success().await {
			Ok(_) => return Err(anyhow!("paused sponsored transaction unexpectedly succeeded")),
			Err(error) => println!("  Rejected as expected: {error}"),
		},
		Err(error) => println!("  Rejected as expected: {error}"),
	}

	let state = query_demo_state(&client, &sponsor_account, &sponsored_user_account).await?;
	print_state("Final state", &state);

	Ok(())
}
