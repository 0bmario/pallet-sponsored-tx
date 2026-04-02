# Pallet Sponsored Tx - Development Commands

polkadot_version := "polkadot-stable2512-2"

# Detect OS and architecture

os := `uname -s | tr '[:upper:]' '[:lower:]'`
arch := `uname -m`
polkadot_sdk_base := "https://github.com/paritytech/polkadot-sdk/releases/download/" + polkadot_version + "/"
darwin_suffix := if os == "darwin" { if arch == "aarch64" { "-aarch64-apple-darwin" } else { "" } } else { "" }

default:
    @just --list

# Build the runtime
build:
    cargo build --release --locked

# Run pallet tests
test:
    cargo test -p pallet-sponsored-tx

# Build-check the Subxt example client
example-check:
    cargo check -p sponsored-tx-subxt-example

# Regenerate benchmark-derived weights for the sponsored tx pallet
benchmark-sponsored-tx:
    # Skip revive fixtures so nested WASM builds do not require solc for this pallet benchmark flow.
    SKIP_PALLET_REVIVE_FIXTURES=1 cargo build --release --locked -p parachain-template-node --features runtime-benchmarks
    # Use the repo-local template so the generated file keeps this crate's WeightInfo contract.
    SKIP_PALLET_REVIVE_FIXTURES=1 ./target/release/parachain-template-node benchmark pallet \
        --chain dev \
        --wasm-execution compiled \
        --pallet pallet_sponsored_tx \
        --extrinsic '*' \
        --steps 50 \
        --repeat 20 \
        --output pallets/sponsored-tx/src/weights.rs \
        --template .maintain/frame-weight-template.hbs

# Run pallet tests with logs
test-verbose:
    RUST_LOG=debug cargo test -p pallet-sponsored-tx -- --nocapture

fmt:
    cargo +nightly fmt -p pallet-sponsored-tx

clippy:
    cargo +nightly clippy -p pallet-sponsored-tx

[private]
_download BIN URL:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .bin
    if [[ -x .bin/{{ BIN }} ]]; then
        echo "{{ BIN }} already exists in .bin/"
        exit 0
    fi
    echo "Downloading {{ BIN }}..."
    trap 'rm -f .bin/{{ BIN }}.tmp' EXIT
    curl --fail -L -o .bin/{{ BIN }}.tmp "{{ URL }}"
    mv .bin/{{ BIN }}.tmp .bin/{{ BIN }}
    chmod +x .bin/{{ BIN }}
    echo "{{ BIN }} downloaded to .bin/{{ BIN }}"

# Download chain-spec-builder and polkadot-omni-node
download-binaries: (_download "chain-spec-builder" polkadot_sdk_base + "chain-spec-builder" + darwin_suffix) (_download "polkadot-omni-node" polkadot_sdk_base + "polkadot-omni-node" + darwin_suffix)
    @echo "All binaries downloaded to .bin/"

# Generate chain spec from the built runtime
chain-spec: download-binaries build
    .bin/chain-spec-builder create -t development \
        --relay-chain paseo \
        --para-id 1000 \
        --runtime ./target/release/wbuild/parachain-template-runtime/parachain_template_runtime.compact.compressed.wasm \
        named-preset development
    @echo "Chain spec generated: chain_spec.json"

# Start omni-node in dev mode
run: chain-spec
    .bin/polkadot-omni-node --chain ./chain_spec.json --dev

# Full setup: download binaries + build + symlinks
setup: download-binaries build
    @ln -sf AGENTS.md CLAUDE.md
    @echo ""
    @echo "Setup complete! Run 'just run' to start the node."
