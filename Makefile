WASM_TARGET = wasm32-wasip1

.PHONY: check release wasm wasm-test test test-wasm test-wasm-cli

check:
	cargo clippy --workspace
	cargo fmt -- --check
	ruff format --check lib/spython/
	deno fmt --check wasm/tests/ wasm/spython.ts

release:
	cargo build --release
	objcopy --remove-section=.eh_frame --remove-section=.eh_frame_hdr target/release/spython

wasm:
	cargo build -p wasm --target $(WASM_TARGET) --profile release-small
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int target/$(WASM_TARGET)/release-small/spython.wasm -o target/$(WASM_TARGET)/release-small/spython.wasm

# Build the WASM binary with the `wasm-backend` feature (adds `run_source`).
# Used by `test-wasm-cli` — never by release/distribution builds.
wasm-test:
	cargo build -p wasm --target $(WASM_TARGET) --profile release-small --features wasm-backend
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int target/$(WASM_TARGET)/release-small/spython.wasm -o target/$(WASM_TARGET)/release-small/spython.wasm

test: test-wasm test-wasm-cli
	cargo test --workspace

test-wasm: wasm
	deno test --allow-read wasm/tests/

# Runs the CLI integration tests against both the native binary and the Deno
# WASM wrapper, asserting identical stdout/stderr after path normalization.
# Requires the wasm32-wasip1 target and Deno on PATH.
test-wasm-cli: wasm-test
	cargo test -p cli --features wasm-backend --test cli
