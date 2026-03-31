# Architecture & Design Review

## Overview

`pallet-sponsored-tx` implements runtime-native sponsored fee payment for first-party clients on a Substrate parachain. A sponsor registers a policy and escrows native balance; an approved signer submits a normal signed extrinsic naming the sponsor, and the runtime charges the sponsor instead of the signer.

The pallet consists of three logical layers:

1. **Sponsor lifecycle pallet** (`lib.rs`, `types.rs`) — registration, budget management, policy, pause/resume, unregister.
2. **Custom payment transaction extension** (`extension.rs`) — replaces `ChargeTransactionPayment` in the runtime's `TxExtension` stack.
3. **Runtime integration** (`runtime/src/lib.rs`, `runtime/src/configs/mod.rs`) — wires the pallet and extension into a parachain template runtime.

## Design Decisions

### 1. Explicit Sponsor Model

The extension carries `sponsor: Option<AccountId>` directly in the signed payload. The caller names exactly who pays.

**Strengths:**

- Deterministic: no runtime search, no ambiguity about who pays.
- Simple failure model: either the named sponsor can pay or validation fails.
- Simple client behavior: the signer knows at construction time who the sponsor is.

**Tradeoff:** Requires the client to know the sponsor account ahead of time. This rules out sponsor discovery or marketplace-style matching, which is the right V1 scope cut.

### 2. Hold-Backed Budgeting

Sponsor funds are native-token balance holds on the sponsor's own account, not a synthetic pool balance in pallet storage.

**Strengths:**

- Budget is real chain money observable through standard `pallet_balances` queries.
- No shadow accounting that could diverge from actual balances.
- Aligns with how other Substrate pallets (staking, democracy) manage reserved funds.

**Tradeoff:** Requires two hold reasons and a release-then-hold dance during prepare/post_dispatch, which is more complex than a simple `StorageMap<AccountId, Balance>`.

### 3. Two-Phase Hold (Budget vs. Pending)

| Hold Reason          | Meaning                                              |
| -------------------- | ---------------------------------------------------- |
| `SponsorshipBudget`  | Available sponsor capacity                           |
| `SponsorshipPending` | Worst-case fee reserved for an in-flight transaction |

The `prepare` phase moves the estimated fee from Budget to Pending. After dispatch, `post_dispatch` slashes the actual fee from Pending and restores the remainder to Budget.

**Strengths:**

- Available and in-flight amounts are independently queryable.
- Prevents a sponsor from withdrawing funds reserved for an in-flight transaction.
- Clean settlement: post_dispatch only consumes funds already isolated for this transaction.

**Tradeoff:** The move between holds is not atomic (release then hold). See Security Review for analysis.

### 4. Reuse of `pallet_transaction_payment` Fee Math

The sponsored path uses `compute_fee` and `compute_actual_fee` from `pallet_transaction_payment` rather than introducing its own fee formula.

**Strengths:**

- Sponsored and unsponsored fees are calculated identically.
- Tip handling, weight-to-fee conversion, and length fee are all inherited.
- Fee multiplier updates apply uniformly.

**Tradeoff:** The pallet is tightly coupled to `pallet_transaction_payment` and `pallet_balances`. This is reasonable for a fee-payment extension.

### 5. Unsponsored Fallback

When `sponsor = None`, the extension delegates entirely to `ChargeTransactionPayment`. Normal signed extrinsics work unchanged.

**Strength:** Sponsorship is additive. The chain does not require sponsorship. The extension replaces the payment slot without breaking the normal payment path.

### 6. Narrow Policy Surface

V1 policy consists of:

- `allowed_callers: BoundedVec<AccountId, MaxAllowedCallers>` — who may use this sponsor
- `max_fee_per_tx: Balance` — per-transaction fee cap

No pallet filters, rate limits, call selectors, or time-based rules.

**Strength:** Minimal attack surface, simple to audit, sufficient for controlled onboarding and app-operated flows.

**Tradeoff:** A single allowlisted caller can drain the entire budget without any rate limiting. The sponsor must trust their allowlisted callers or monitor budget off-chain.

## Storage Design

```
Sponsors: StorageMap<AccountId, SponsorState>
```

Single storage map. No double maps, no auxiliary indices. Budget is tracked via balance holds, not storage values.

**Observation:** This is clean and minimal. The downside is that there is no on-chain index of "all sponsors" or "all callers for a sponsor" beyond iterating the map or reading individual entries.

## Extension Stack Position

```rust
TxExtension = StorageWeightReclaim<Runtime, (
    AuthorizeCall,
    CheckNonZeroSender,
    CheckSpecVersion,
    CheckTxVersion,
    CheckGenesis,
    CheckEra,
    CheckNonce,
    CheckWeight,
    SponsoredChargeTransactionPayment,  //  replaces ChargeTransactionPayment
    CheckMetadataHash,
)>;
```

The sponsored extension occupies the same slot that `ChargeTransactionPayment` would. It is wrapped by `StorageWeightReclaim` and follows all standard system checks. This is the correct position.

## Transaction Lifecycle

```
validate → prepare → dispatch → post_dispatch
```

| Phase           | Sponsored Path                                                                                                                     |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `validate`      | Check sponsor exists, active, caller allowed, fee within cap, budget sufficient. Return priority.                                  |
| `prepare`       | Move estimated fee from `SponsorshipBudget` hold to `SponsorshipPending` hold.                                                     |
| dispatch        | Normal call execution. Extension is not involved.                                                                                  |
| `post_dispatch` | Compute actual fee. Slash from pending hold. Route credit to `FeeDestination`. Restore unused estimate to budget hold. Emit event. |

## Runtime Wiring

- Pallet index 12, in the monetary pallets section.
- `MaxSponsoredCallers = 32` — reasonable for V1.
- `FeeDestination = ()` — fees are burned. See Code Quality Review for production implications.
- `SponsorshipHoldReasonConverter` maps pallet hold reasons to `RuntimeHoldReason`.
- Weights use `pallet_sponsored_tx::weights::SubstrateWeight<Runtime>` (placeholder values).

## Client Integration

The Subxt example (`examples/subxt-sponsor-client/`) demonstrates the custom extension encoding. This is necessary because generic clients (Polkadot.js, wallets) do not automatically know how to encode `SponsoredChargeTransactionPayment`.

The example uses `transaction_extensions::AnyOf` to compose the full extension stack and includes a `SponsoredParamsBuilder` for ergonomic construction.

## Summary Assessment

The architecture is sound for its stated scope. The key design choices (explicit sponsor, hold-backed budgeting, two-phase holds, fee math reuse, unsponsored fallback) are well-reasoned and appropriate for a V1 first-party-client sponsorship pallet. The implementation is narrow by design, which limits both the attack surface and the feature set.
