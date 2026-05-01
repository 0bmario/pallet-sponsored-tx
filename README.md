# pallet-sponsored-tx

A FRAME pallet for runtime-native sponsored fee payment on Polkadot SDK. Sponsors register a policy and escrow native tokens; approved signers submit transactions where the sponsor pays the fee instead of the signer. Built around the `TransactionExtension` lifecycle (validate / prepare / post_dispatch) with a two-hold model that separates available budget from in-flight reservations.


## Prerequisites

```sh
# just (command runner) - https://github.com/casey/just
cargo install just

# rust toolchain
rustup install 1.88
rustup default 1.88
rustup target add wasm32-unknown-unknown --toolchain 1.88
rustup component add rust-src --toolchain 1.88

# nightly (for fmt/clippy)
rustup install nightly
```

## Setup

Download binaries and build the runtime:

```sh
just setup
```

## Run the Node Locally

```sh
just run
```

This builds the runtime, generates a chain spec, and starts the omni-node in `--dev` mode (clean state on each restart).

## Subxt Demo

Scripted Rust client that walks through the sponsored transaction lifecycle with Subxt. This is the canonical demo because it shows the custom `SponsoredChargeTransactionPayment` transaction extension being encoded by a first-party client.

```sh
# 1. start the node (separate terminal)
just run

# 2. run the Subxt demo
just subxt-demo
```

The demo uses dev accounts (Alice/Bob/Charlie) and `SPONSORED_TX_RPC_URL` if set, otherwise `ws://127.0.0.1:9944`.

**Flow:** Register Alice as sponsor -> show Alice's budget hold -> submit a sponsored `System::remark` from Bob (Alice pays the fee) -> print `SponsoredTransactionFeePaid` -> show pending hold returns to zero -> pause Alice -> show sponsored submission rejected while paused.

## Development

```sh
just test          # run pallet tests
just fmt           # format
just clippy        # lint
just --list        # see all commands
```

## Known Limitations (V1)

- **Trusted callers only** — sponsors define an allowlist; no rate limiting or call filtering (see S-04, S-05 in the security review).
- **Placeholder weights** — benchmarking is deferred; post-dispatch uses hand-counted storage estimates.
- **FeeDestination burns fees** — both the mock and runtime route fee credit to `()`. A production deployment should route to treasury or block author.
