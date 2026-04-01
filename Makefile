WASM_TARGET = wasm32-wasip1

.PHONY: check release wasm test test-wasm

check:
	cargo clippy --workspace
	cargo fmt -- --check
	ruff format --check lib/spython/
	deno fmt --check wasm/tests/

release:
	cargo build --release
	objcopy --remove-section=.eh_frame --remove-section=.eh_frame_hdr target/release/spython

wasm:
	cargo build -p wasm --target $(WASM_TARGET) --profile release-small
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int target/$(WASM_TARGET)/release-small/spython.wasm -o target/$(WASM_TARGET)/release-small/spython.wasm

test: test-wasm
	cargo test --workspace

test-wasm: wasm
	deno test --allow-read wasm/tests/
