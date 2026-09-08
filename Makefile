WASM_TARGET = wasm32-wasip1

.PHONY: release wasm wasm-test test test-rs test-wasm test-wasm-cli test-images check

release:
	cargo build --release
	objcopy --remove-section=.eh_frame --remove-section=.eh_frame_hdr target/release/spython

wasm:
	cargo build -p wasm --target $(WASM_TARGET) --profile release-small
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int target/$(WASM_TARGET)/release-small/spython.wasm -o target/$(WASM_TARGET)/release-small/spython.wasm

# Like wasm, plus the `wasm-backend` feature, which adds the `run_source`,
# `check_source` and `test_source` the dual-run needs. Used by test-wasm-cli,
# never by a release build.
wasm-test:
	cargo build -p wasm --target $(WASM_TARGET) --profile release-small --features wasm-backend
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int target/$(WASM_TARGET)/release-small/spython.wasm -o target/$(WASM_TARGET)/release-small/spython.wasm

test: test-rs test-wasm test-wasm-cli test-images

test-rs:
	cargo test --workspace

test-wasm: wasm
	deno test --allow-read wasm/tests/

# Runs the CLI integration tests against both the native binary and the Deno
# WASM wrapper, asserting both backends produce identical output. Requires
# the wasm32-wasip1 target and Deno. CI should enable this only on one
# platform (e.g., Linux) to avoid redundant WASM runs across the matrix.
test-wasm-cli: wasm-test
	cargo test -p spython --features wasm-backend --test cli

# The image snapshots are long and many, so they carry `#[ignore]`.
test-images:
	cargo test --workspace -- --ignored

check:
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy --target $(WASM_TARGET) -p wasm -- -D warnings
	cargo fmt -- --check
	ruff format --check lib/spython/
	deno fmt --check wasm/tests/ wasm/spython.ts
