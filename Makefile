WASM_TARGET = wasm32-wasip1
WASM_BIN    = target/$(WASM_TARGET)/release/spython.wasm
WEB_DIR     = web
DIST_DIR    = dist
BUILD_DIR   = build

DIST_FILES = \
	$(DIST_DIR)/engine.wasm \
	$(DIST_DIR)/index.html \
	$(DIST_DIR)/server.py

.PHONY: all serve test test-web test-rs check clean

all: $(DIST_FILES)

serve: $(DIST_FILES)
	( timeout 3 bash -c 'until curl -s http://localhost:8000 > /dev/null; do sleep 0.5; done; xdg-open http://localhost:8000' ) & \
	cd dist && python server.py

# WASM binary

$(DIST_DIR)/engine.wasm: $(WASM_BIN) | $(DIST_DIR)
	wasm-strip $< -o $@
	wasm-opt -Oz --enable-bulk-memory --enable-mutable-globals --enable-sign-ext --enable-nontrapping-float-to-int $@ -o $@

$(DIST_DIR)/server.py: $(WEB_DIR)/server.py | $(DIST_DIR)
	cp $< $@

RUST_SRCS = Cargo.toml \
	$(wildcard spython-core/src/*.rs) \
	$(wildcard src/*.rs) \
	$(wildcard wasm/src/*.rs)

$(WASM_BIN): $(RUST_SRCS)
	cargo build -p spython-wasm --target $(WASM_TARGET) --release

# TypeScript bundles (intermediate, used by inline.ts)

$(BUILD_DIR)/worker.js: $(WEB_DIR)/worker.ts $(WEB_DIR)/worker_channel.ts $(WEB_DIR)/ui_channel.ts $(WEB_DIR)/env.ts $(WEB_DIR)/wasi.ts | $(BUILD_DIR)
	deno bundle $(WEB_DIR)/worker.ts -o $@

$(BUILD_DIR)/ui.js: $(WEB_DIR)/ui.ts $(WEB_DIR)/ui_channel.ts $(WEB_DIR)/ansi.ts | $(BUILD_DIR)
	deno bundle $(WEB_DIR)/ui.ts -o $@

$(BUILD_DIR)/codeflask.min.js: | $(BUILD_DIR)
	curl -sL https://unpkg.com/codeflask/build/codeflask.min.js -o $@

$(BUILD_DIR)/prism-python.min.js: | $(BUILD_DIR)
	curl -sL https://unpkg.com/prismjs/components/prism-python.min.js -o $@

# Inline everything into a single index.html

$(DIST_DIR)/index.html: $(WEB_DIR)/spython.html $(BUILD_DIR)/ui.js $(BUILD_DIR)/worker.js $(BUILD_DIR)/codeflask.min.js $(BUILD_DIR)/prism-python.min.js $(WEB_DIR)/inline.ts | $(DIST_DIR)
	deno run --allow-read --allow-write $(WEB_DIR)/inline.ts

# Tests

test: test-rs test-web

test-rs:
	cargo test

$(BUILD_DIR)/engine.wasm: $(DIST_DIR)/engine.wasm | $(BUILD_DIR)
	ln -sf ../$(DIST_DIR)/engine.wasm $@

test-web: $(DIST_DIR)/engine.wasm $(BUILD_DIR)/worker.js $(BUILD_DIR)/engine.wasm
	deno test $(WEB_DIR)/channel_test.ts
	deno test $(WEB_DIR)/wasi_test.ts
	deno test $(WEB_DIR)/ansi_test.ts
	deno test --no-check --allow-read $(WEB_DIR)/test.ts

# Check

check:
	cargo clippy -- -D warnings
	cargo fmt -- --check
	deno fmt --check
	ruff check .
	ruff format --check .

# Freeze allowlist

FREEZE_ALLOWLIST = crates/RustPython/Lib/freeze_allowlist.txt
FREEZE_SEEDS = ast dataclasses enum encodings typing spython

freeze-allowlist:
	python3 scripts/find_stdlib_deps.py crates/RustPython/Lib $(FREEZE_SEEDS) > $(FREEZE_ALLOWLIST)

# Utility

$(DIST_DIR):
	mkdir -p $@

$(BUILD_DIR):
	mkdir -p $@

clean:
	rm -rf $(DIST_DIR) $(BUILD_DIR)
