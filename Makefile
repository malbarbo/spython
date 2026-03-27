.PHONY: release wasm

release:
	cargo build --release
	objcopy --remove-section=.eh_frame --remove-section=.eh_frame_hdr target/release/spython

wasm:
	cd wasm && cargo build --target wasm32-wasip1 --release
	wasm-strip wasm/target/wasm32-wasip1/release/spython.wasm
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int wasm/target/wasm32-wasip1/release/spython.wasm -o wasm/target/wasm32-wasip1/release/spython.wasm
