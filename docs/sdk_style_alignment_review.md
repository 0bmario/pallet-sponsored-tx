# SDK Style Alignment Review

## Purpose

This note reviews the current `pallet-sponsored-tx` implementation against local `polkadot-sdk` pallet style using the `pwalk` vault workflow. The goal is not to revisit product design or runtime behavior. The goal is to identify where the codebase already matches SDK patterns and where it should be tightened to feel like a native FRAME pallet written in the same style as the SDK.

## Pwalk Workflow Used

The vault was checked first and did not require refresh.

Command used:

```bash
cargo run --manifest-path /Users/mmo/fun/pkdot/pwalk/Cargo.toml -- --config /Users/mmo/fun/pkdot/pwalk/pwalk.toml doctor --json
```

Chosen package notes:

- `pallet-transaction-payment`
  - best reference for fee-payment semantics, extension-driven payment handling, and crate-level documentation style
- `pallet-meta-tx`
  - best reference for transaction-extension-oriented pallet structure and extension module style
- `pallet-balances`
  - best reference for hold-backed accounting style, rich pallet documentation, and public API documentation patterns

SDK files opened next:

- `substrate/frame/transaction-payment/src/lib.rs`
- `substrate/frame/meta-tx/src/lib.rs`
- `substrate/frame/meta-tx/src/extension.rs`
- `substrate/frame/balances/src/lib.rs`

## What The SDK Style Consistently Looks Like

### 1. Strong Crate-Level Documentation

The SDK pallets usually start with:

- SPDX and license header
- a crate-level `//!` overview
- explicit sections such as:
  - overview
  - terminology
  - usage
  - implementation details
  - examples

This is most visible in:

- `pallet-transaction-payment`
- `pallet-meta-tx`
- `pallet-balances`

The crate docs explain the "why" and "shape" of the pallet before the reader reaches the implementation.

### 2. Public API Is Documented, Not Just Present

SDK pallets tend to document:

- public structs
- public type aliases
- config associated types
- storage items
- events
- errors
- dispatchables

The documentation is usually concise, but it makes the public surface self-describing in generated docs and when reading source.

### 3. The Pallet Module Is Self-Contained

In the SDK, the pallet module usually imports its own `pallet_prelude::*` and `frame_system::pallet_prelude::*` locally. Public types and helpers that belong outside the pallet module are documented at the crate root. This makes it easier to read the pallet in isolation and keeps the macro-generated section idiomatic.

### 4. Comments Explain Invariants Or Phase Boundaries

SDK comments are usually not line-by-line narration. They tend to explain:

- why something exists
- what invariant a block of code protects
- what phase transition is happening
- what assumption a caller or runtime must satisfy

This is especially visible in payment and extension-related code.

### 5. Benchmarks And Weights Are First-Class

SDK pallets usually treat:

- `benchmarking.rs`
- `weights.rs`
- `mock.rs`
- `tests.rs`

as part of the normal pallet shape, not as optional afterthoughts. Even when a pallet is simple, the file layout communicates that benchmarking and generated weights are expected.

### 6. Examples And Tests Reinforce The Docs

Reference pallets often use tests or embedded examples to show intended usage. The code structure and docs reinforce each other instead of living separately.

## Where `pallet-sponsored-tx` Already Matches Well

### 1. File Layout Is In The Right Shape

The pallet already has the expected FRAME-style file split:

- `lib.rs`
- `extension.rs`
- `types.rs`
- `weights.rs`
- `benchmarking.rs`
- `mock.rs`
- `tests.rs`

That is the right overall structure and already feels close to SDK practice.

### 2. The Product Shape Is Narrow And Coherent

The public surface is intentionally small:

- sponsor lifecycle dispatchables
- sponsor state and policy types
- a focused custom payment extension

This is aligned with how SDK pallets usually expose a tight, purposeful API.

### 3. Tests Are Behavior-Oriented

The test names and scenarios are already structured around behavior:

- registration
- policy validation
- prepare and post-dispatch settlement
- unsponsored fallback

That is directionally aligned with SDK test style.

## Style Gaps After Documentation Pass

### 1. Crate-Level Documentation

Status:

- `pallets/sponsored-tx/src/lib.rs` now has crate-level `//!` docs
- docs explain sponsor lifecycle, two-hold budget model, transaction extension behavior, and V1 scope
- this now matches the broad SDK expectation that the top of the pallet frames the runtime semantics

Remaining improvement:

- examples could be added later if the pallet grows beyond the current first-party client scope

### 2. Public Types And Public API Documentation

Status:

- `SponsorPolicy`, `SponsorState`, `SponsoredChargeTransactionPayment`, and extension helper methods are documented
- config associated types, storage, events, errors, and dispatchables now have concise doc comments
- this is much closer to generated-doc quality expected from SDK-style pallets

Remaining improvement:

- once benchmarks and examples settle, consider `#![deny(missing_docs)]` to keep coverage from regressing

### 3. Extension Lifecycle Needs Explicit Invariant Comments

Status:

- `extension.rs` now has comments around unsponsored fallback, signed-origin restriction, prepare reservation, post-dispatch clamping, settlement weight, fee/tip routing, and pending-to-budget restoration
- the key `validate -> prepare -> post_dispatch` phase boundaries are easier to review

Remaining improvement:

- comments should stay focused on invariants; avoid expanding into line-by-line narration

### 4. Pallet Module Imports Are Less Idiomatic Than SDK Style

Current gap:

The current pallet uses more crate-root imports and `use super::*` plumbing than a typical SDK pallet that keeps the macro section more locally self-contained.

This is not wrong, but it feels less like the reference pallets.

Recommendation:

- tighten imports inside the pallet module
- prefer the familiar `frame_support::pallet_prelude::*` and `frame_system::pallet_prelude::*` style locally where practical
- keep crate-root exports and helper aliases documented

### 5. Benchmarking And Weights Are Structurally Present But Not Yet SDK-Grade

Current gap:

- `benchmarking.rs` explicitly says benchmarking is deferred
- `weights.rs` contains manual placeholder values
- sponsored post-dispatch settlement uses hand-counted reads/writes rather than generated benchmark output

Why it matters:

- the file layout is correct
- the implementation is not yet at the normal SDK completeness bar
- proof-size weights matter for parachain PoV limits

Recommendation:

- treat benchmark coverage and generated weights as a required style-completion step
- do not leave manual weights as the long-term state

### 6. The Example Client Is Useful But Demo-Scoped

Current state:

The Subxt example proves the path and now has a useful module-level explanation. It is intentionally scripted for a local demo:

- hard-coded dev accounts and budget values
- no production idempotency or retry logic
- no separate generic unsponsored user transaction beyond Alice's unsponsored registration

Recommendation:

- keep it small for recording
- add flags/idempotency only if it becomes a reusable operator tool

## Recommended Cleanup Order

Current remaining cleanup order:

1. Finish benchmarking and replace manual weights with benchmark-derived weights.
2. Add proof-size weights.
3. Improve example robustness only if it becomes more than a demo.
4. Tighten pallet-module imports only if it improves readability.
5. Consider `#![deny(missing_docs)]` after the public API stabilizes.

## What Not To Change In The Name Of Style

A style pass should not silently alter:

- runtime behavior
- sponsor validation rules
- budget accounting model
- signed payload shape
- runtime integration order

Those are design decisions. The style pass should make them easier to understand, not different.

## Bottom Line

The implementation already has the right macro-level structure for a FRAME pallet, and the recent documentation pass closed the largest SDK-style readability gaps. The remaining gap is production completeness: benchmarks, generated weights with proof size, and operator-grade client hardening if the demo client is reused.

In practice, the best next step is a non-functional production-readiness pass focused on:

- benchmark and weight completeness
- proof-size accounting
- client hardening only beyond demo scope

That is what will make `pallet-sponsored-tx` feel much closer to a native SDK pallet.
