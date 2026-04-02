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

## Main Style Gaps To Fix In A Follow-Up Pass

### 1. Crate-Level Documentation Is Now In Place

Current state:

- `pallets/sponsored-tx/src/lib.rs` now opens with crate-level `//!` documentation
- the overview explains the pallet API, budget model, transaction extension, and V1 scope
- the top-level docs now frame the code the way SDK reviewers expect

Follow-up:

- keep the crate docs in sync as the pallet evolves
- eventually enforce the docs bar with `#[deny(missing_docs)]` when the crate is ready

### 2. Public Types And Most Public API Are Now Documented

Current state:

- `SponsorPolicy`, `SponsorState`, `SponsoredChargeTransactionPayment`, the key helper methods, config items, storage, events, errors, and dispatchables are now documented
- generated docs and source reading are materially better than in the original review snapshot

Remaining gap:

- documentation quality is still convention-based rather than compiler-enforced
- a future cleanup pass can tighten any remaining helper/internal API docs once the public surface settles

### 3. Extension Lifecycle Needs Explicit Invariant Comments

Current gap:

The extension logic is correct, but the code in `extension.rs` does not explain the core invariants clearly enough.

The most important missing explanations are:

- why sponsored validation is restricted to signed origins
- why `prepare` moves funds from budget to pending
- why `post_dispatch` may clamp actual fee to estimate
- why pending is restored back into budget after slashing
- how sponsored and unsponsored paths intentionally differ

Why it matters:

- transaction extensions are harder to read than normal dispatchables
- this is the least familiar part of the implementation for most reviewers
- a few targeted comments would remove most of the cognitive load

Recommendation:

- add comments at the major phase boundaries
- do not narrate every line
- document the invariants and the intent of each transition

### 4. Pallet Module Imports Are Less Idiomatic Than SDK Style

Current gap:

The current pallet uses more crate-root imports and `use super::*` plumbing than a typical SDK pallet that keeps the macro section more locally self-contained.

This is not wrong, but it feels less like the reference pallets.

Recommendation:

- tighten imports inside the pallet module
- prefer the familiar `frame_support::pallet_prelude::*` and `frame_system::pallet_prelude::*` style locally where practical
- keep crate-root exports and helper aliases documented

### 5. Events And Errors Are Documented

Current state:

- event and error variants now carry per-variant doc comments
- the metadata-facing surface is much closer to normal SDK expectations

Follow-up:

- keep new variants documented as they are introduced

### 6. Dispatchable Benchmarking And Weights Are Now SDK-Grade

Current state:

- `benchmarking.rs` now contains FRAME v2 benchmarks for all seven dispatchables
- `weights.rs` is generated from benchmark output and includes proof-size estimates
- `register_sponsor` and `set_policy` now expose the allowlist-length component explicitly in the weight interface

Remaining gap:

- the sponsored extension settlement path still uses a hand-written placeholder post-dispatch weight

Recommendation:

- keep the checked-in weights regenerated through `just benchmark-sponsored-tx`
- keep using the repo-local Handlebars template rather than the CLI default output
- treat extension benchmarking as the remaining completeness step

### 7. The Example Client Is Useful But Under-Documented

Current gap:

The Subxt example proves the path, but it does not yet read like an SDK-quality example:

- minimal top-level explanation
- no guidance on why a custom Subxt extension is needed
- no explanation of the payload shape relative to runtime metadata

Recommendation:

- add a short module-level comment at the top of the example
- explain that this example exists because generic clients do not automatically know how to encode the custom transaction extension

## Recommended Cleanup Order

This is the order I would use in the next dedicated style/completeness pass.

1. Add `#[deny(missing_docs)]` once the crate is ready to enforce it.
2. Tighten pallet-module import style where it improves readability.
3. Improve the example’s top-level explanatory comments if it grows beyond the minimal happy path.
4. Benchmark the sponsored extension settlement path and keep the generated dispatchable weights current.

## What Not To Change In The Name Of Style

A style pass should not silently alter:

- runtime behavior
- sponsor validation rules
- budget accounting model
- signed payload shape
- runtime integration order

Those are design decisions. The style pass should make them easier to understand, not different.

## Bottom Line

The implementation already has the right macro-level structure for a FRAME pallet. The documentation and invariant-signposting gaps from the original review are much smaller now. The main remaining completeness gap is the custom extension settlement weight rather than the pallet architecture or dispatchable benchmarking.

In practice, the best next step is a non-functional cleanup pass focused on:

- extension-weight completeness
- docs enforcement
- any remaining style-only import cleanup

That is what will make `pallet-sponsored-tx` feel much closer to a native SDK pallet.
