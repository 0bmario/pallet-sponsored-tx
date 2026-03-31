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

## development

```sh
just test          # run pallet tests
just fmt           # format
just clippy        # lint
just --list        # see all commands
```
