## prerequisites

```sh
rustup install 1.88
rustup default 1.88
rustup target add wasm32-unknown-unknown --toolchain 1.88-aarch64-apple-darwin
rustup component add rust-src --toolchain 1.88-aarch64-apple-darwin
```

## chain spec builder and omninode

```sh
cargo install --locked staging-chain-spec-builder@16.0.0
cargo install --locked polkadot-omni-node@0.13.2
```

## compile the runtime

```sh
cargo build --release --locked
```

## verify the wasm

```sh
ls -la ./target/release/wbuild/parachain-template-runtime/
```

# Run the node locally

- generate chain spec:

```sh
chain-spec-builder create -t development \
--relay-chain paseo \
--para-id 1000 \
--runtime ./target/release/wbuild/parachain-template-runtime/parachain_template_runtime.compact.compressed.wasm \
named-preset development
```

- Start the Omni Node with the generated chain spec:

```sh
polkadot-omni-node --chain ./chain_spec.json --dev
```

- The `--dev` option does the following:
  - Deletes all active data (keys, blockchain database, networking information) when stopped.
  - Ensures a clean working state each time you restart the node.
