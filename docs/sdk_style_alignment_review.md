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

### 1. Missing Crate-Level Documentation

Current gap:

- `pallets/sponsored-tx/src/lib.rs` starts directly with `#![cfg_attr(...)]`
- there is no crate-level `//!` overview
- there is no explanation of terminology such as sponsor, budget hold, pending hold, or sponsored payment path

Why it matters:

- this is the first thing a reviewer sees
- SDK pallets rely on these top-level docs to frame the code
- this pallet has non-trivial semantics and needs that context

Recommendation:

- add crate-level docs in the style of `pallet-meta-tx` and `pallet-transaction-payment`
- include:
  - overview
  - pallet API
  - implementation details
  - settlement model
  - first-party client scope

### 2. Missing Documentation On Public Types And Public API

Current gap:

- `SponsorPolicy`
- `SponsorState`
- `SponsoredChargeTransactionPayment`
- public helper methods like `new`, `tip`, and `sponsor`
- config associated types
- storage item `Sponsors`
- event variants
- error variants
- dispatchables

are mostly undocumented.

Why it matters:

- FRAME pallets are consumed as public runtime components
- public docs are part of the API quality bar
- the event and error surface should be readable without reverse-engineering implementation

Recommendation:

- add doc comments to every public type and public pallet item
- use short, direct docs, not long prose everywhere

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

### 5. Events And Errors Need Per-Variant Docs

Current gap:

Event and error variants are readable by name, but they do not carry SDK-style doc comments.

Why it matters:

- SDK pallets usually document what each event means and what each error condition represents
- this matters for downstream users, docs, and generated metadata readers

Recommendation:

- add one-line docs to each event and error variant
- reserve longer comments for the few variants that need explanation

### 6. Benchmarking And Weights Are Structurally Present But Not Yet SDK-Grade

Current gap:

- `benchmarking.rs` explicitly says benchmarking is deferred
- `weights.rs` contains manual placeholder values

Why it matters:

- the file layout is correct
- the implementation is not yet at the normal SDK completeness bar

Recommendation:

- treat benchmark coverage and generated weights as a required style-completion step
- do not leave manual weights as the long-term state

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

This is the order I would use in a dedicated style pass.

1. Add crate-level docs to `pallets/sponsored-tx/src/lib.rs`.
2. Add doc comments to public types in `types.rs` and `extension.rs`.
3. Add doc comments to config items, storage, events, errors, and dispatchables in `lib.rs`.
4. Add targeted invariant comments in `extension.rs` around validate, prepare, and post-dispatch.
5. Tighten pallet-module import style where it improves readability.
6. Improve the example’s top-level explanatory comments.
7. Finish benchmarking and replace manual weights with benchmark-derived weights.

## What Not To Change In The Name Of Style

A style pass should not silently alter:

- runtime behavior
- sponsor validation rules
- budget accounting model
- signed payload shape
- runtime integration order

Those are design decisions. The style pass should make them easier to understand, not different.

## Bottom Line

The implementation already has the right macro-level structure for a FRAME pallet. The main gap is not architecture. The main gap is that the code does not yet explain itself with the same level of documentation and invariant-signposting that SDK pallets usually provide.

In practice, the best next step is a non-functional cleanup pass focused on:

- crate docs
- public API docs
- invariant comments
- benchmark and weight completeness

That is what will make `pallet-sponsored-tx` feel much closer to a native SDK pallet.
