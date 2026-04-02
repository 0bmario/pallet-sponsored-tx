# Sponsored Native Fees V1: Implementation Status

## What Is Implemented

### Pallet (`pallets/sponsored-tx/`)

- Sponsor registration with initial budget hold
- Budget increase and decrease
- Policy update (allowlist + per-tx fee cap)
- Pause, resume, and unregister
- Sponsor state storage (`SponsorState`, `SponsorPolicy`)
- Hold-backed budgeting with two hold reasons: `SponsorshipBudget` (long-lived) and `SponsorshipPending` (per-tx)

### Transaction Extension (`extension.rs`)

- `SponsoredChargeTransactionPayment<T>` with payload: `tip` + `sponsor: Option<AccountId>`
- `sponsor = None` falls back to normal payment path
- `sponsor = Some(...)` activates sponsored validation (policy, allowlist, fee cap, budget check) and settlement (slash pending, route credit, restore unused to budget)
- Sponsored post-dispatch now returns a non-zero placeholder weight for settlement overhead

### Runtime (`runtime/src/configs/mod.rs`)

- Pallet added to runtime, only the payment extension slot replaced
- `RuntimeHoldReason` includes composite enum variant
- Custom hold reason converter
- `MaxSponsoredCallers` constant
- `FeeDestination = ()`, so sponsored fees still burn alongside regular native fees in this pass

### Benchmarking & Weights

- FRAME v2 dispatchable benchmarks for all seven pallet calls
- Runtime benchmark registry includes `pallet_sponsored_tx`
- `weights.rs` is generated from benchmark output with non-zero proof-size estimates
- `register_sponsor` and `set_policy` weights scale with allowlist length
- Supported regeneration path: `just benchmark-sponsored-tx`

### Client Example (`examples/subxt-sponsor-client/`)

- Custom Subxt extension encoder for `SponsoredChargeTransactionPayment`
- Registers Alice as sponsor, allowlists Bob/Charlie, submits sponsored `System.remark`
- Workspace-integrated and compile-checkable via `cargo check -p sponsored-tx-subxt-example`

### Tests (`pallets/sponsored-tx/src/tests.rs`)

- Sponsor lifecycle, policy updates, validation rejection paths, settlement, tip handling, sequential same-sponsor flows, pending-budget guard, unsponsored fallback

## Design Rationale

See `architecture_review.md` for the full design rationale (explicit sponsor, real holds, two-hold model, fee math reuse, unsponsored fallback, narrow policy surface).

## Verified

Local omni-node run with Subxt example successfully registered sponsor, submitted sponsored tx, and emitted:

```
SponsoredTransactionFeePaid { sponsor: Alice, signer: Bob, actual_fee: 1_715_898_613, tip: 0 }
```

Polkadot.js Apps confirmed correct storage and event decoding.

## Known Gaps

1. **Extension benchmarking deferred** — dispatchable weights are benchmark-derived, but the sponsored post-dispatch settlement path still uses a placeholder database weight until dedicated extension benchmarks exist.
2. **No generic wallet support** — write-side submission requires the custom Subxt client. Read-side works in Polkadot.js Apps.
3. **No broader policy engine** — no rate limits, call filters, sponsor discovery, multi-sponsor, cooldown withdrawals, or asset-based fees. Intentional V1 scope cut.
4. **Fee routing unchanged** — sponsored fees intentionally keep the runtime's current burned-fee behavior in this hardening pass.
5. **Example is minimal** — proves the path but not operator-hardened or idempotent.
