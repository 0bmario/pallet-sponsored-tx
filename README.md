## prerequisites

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

## setup

Download binaries and build the runtime:

```sh
just setup
```

## run the node locally

```sh
just run
```

This builds the runtime, generates a chain spec, and starts the omni-node in `--dev` mode (clean state on each restart).

## web demo (PAPI)

Interactive browser UI that walks through the sponsored transaction lifecycle using [Polkadot-API (PAPI)](https://papi.how).

```sh
# 1. start the node (separate terminal)
just run

# 2. install deps and generate PAPI types (requires running node)
just demo-install

# 3. start the Vite dev server
just demo
```

Open the printed URL (default `http://localhost:5173`). The demo uses dev accounts (Alice/Bob/Charlie) so no browser wallet extension is needed.

**Flow:** Register Alice as sponsor -> submit a sponsored `System::remark` from Bob (Alice pays the fee) -> adjust budget, pause/resume, unregister.

## development

```sh
just test          # run pallet tests
just fmt           # format
just clippy        # lint
just --list        # see all commands
```
