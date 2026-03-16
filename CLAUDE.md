# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## What is spython

`spython` is a Python interpreter with integrated type checking aimed at
students. It enforces complete type annotations before running code — useful for
teaching typed Python.

## Build & Run

```bash
# Build
cargo build

# Build optimized
cargo build --release

# Run a Python script (with type checking)
cargo run -- script.py

# Start REPL (no type checking)
cargo run
```

**After every modification**, run these commands and fix any failures before
considering the task done:

```bash
make check     # clippy, rustfmt, deno fmt
make test      # cargo test + all web tests
make test-web  # just the web/WASM integration tests (subset of make test)
```

Manual testing is also done with `.py` files in the repo root (e.g. `a.py`,
`simple.py`, `x.py`, etc.).

## Architecture

The project is a single Rust binary (`src/`) with two local crate dependencies:

- `crates/RustPython` — the Python interpreter used for execution
- `crates/ruff` — provides the `ty` type checker and the `ruff_python_*`
  AST/parser crates

**Vendored crates policy**: Avoid modifying `crates/RustPython` and
`crates/ruff` whenever possible. Prefer hooking into their public APIs from
`spython-core` or `wasm/`. Changes to vendored crates are harder to track and
complicate future upstream updates.

Known necessary exceptions:

- `crates/RustPython/crates/vm/src/vm/mod.rs` — the wasm32 branch of
  `check_signals` was an unconditional no-op. It now declares `check_interrupt`
  as a WASM env import and raises `KeyboardInterrupt` when the JS host sets the
  SharedArrayBuffer interrupt flag. There is no public API hook point in the
  eval loop, so this change cannot be avoided.

**Execution pipeline** (see `src/main.rs:run_checked`):

1. Resolve the given `.py` file to an absolute path and collect all transitively
   imported local Python files (`collect_imports_recursive`).
2. Build a `ty` `ProjectDatabase` from those files (`build_db`).
3. Run spython's custom annotation checker (`src/checker.rs`) — errors if any
   parameter, return type, or class attribute annotation is missing.
4. If annotations pass, run ty's type checker (`db.check()`).
5. If type checking passes, execute the script with RustPython.

**Source files:**

- `src/main.rs` — CLI, pipeline orchestration, import resolution
- `src/checker.rs` — AST walker that checks for missing annotations
- `src/lints.rs` — Defines three custom lint rules using `declare_lint!`:
  `MISSING_PARAMETER_ANNOTATION`, `MISSING_RETURN_ANNOTATION`,
  `MISSING_ATTRIBUTE_ANNOTATION`

## Custom Lint Rules

The three lint rules in `src/lints.rs` use ty's `declare_lint!` macro. They are
checked before ty's own type checker runs, so annotation errors are shown first
and ty errors only appear once annotations are complete.

Rules enforced:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function/method must have a return type annotation
- Every class-body variable assignment must have a type annotation
