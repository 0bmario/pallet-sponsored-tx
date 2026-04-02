#[cfg(feature = "std")]
fn configure_wasm_build_env() {
	// Benchmark/node builds do not need revive's contract fixtures, and skipping them avoids a
	// hard `solc` dependency in the nested WASM build.
	if std::env::var("SKIP_PALLET_REVIVE_FIXTURES").is_err() {
		std::env::set_var("SKIP_PALLET_REVIVE_FIXTURES", "1");
	}
}

#[cfg(all(feature = "std", feature = "metadata-hash"))]
#[docify::export(template_enable_metadata_hash)]
fn main() {
	configure_wasm_build_env();
	substrate_wasm_builder::WasmBuilder::init_with_defaults()
		.enable_metadata_hash("UNIT", 12)
		.build();
}

#[cfg(all(feature = "std", not(feature = "metadata-hash")))]
fn main() {
	configure_wasm_build_env();
	substrate_wasm_builder::WasmBuilder::build_using_defaults();
}

/// The wasm builder is deactivated when compiling
/// this crate for wasm to speed up the compilation.
#[cfg(not(feature = "std"))]
fn main() {}
