# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## What is spython

`spython` is a Python interpreter with integrated type checking aimed at
students. It enforces complete type annotations before running code — useful for
teaching typed Python.

## Setup

```bash
# Clone with submodules (crates/RustPython and crates/ruff)
git clone --recurse-submodules <repo-url>

# Or, if already cloned:
git submodule update --init --recursive
```

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

**Execution pipeline** (see `src/main.rs:run_checked`; level defaults to 1):

1. Resolve the given `.py` file to an absolute path and collect all transitively
   imported local Python files (`collect_imports_recursive`).
2. Build a `ty` `ProjectDatabase` from those files (`build_db`).
3. Run spython's custom checker (`spython-core/src/checker.rs`) — checks
   annotations and forbidden constructs based on the teaching level.
4. If the checker passes, run ty's type checker (`db.check()`).
5. If type checking passes, execute the script with RustPython.

**Source files:**

- `src/main.rs` — CLI, pipeline orchestration, import resolution
- `spython-core/src/checker.rs` — AST walker: annotation checks + construct
  restrictions
- `spython-core/src/lints.rs` — Lint rule declarations using `declare_lint!`
- `spython-core/src/doctest_runner.py` — Minimal doctest runner (avoids stdlib
  `doctest` which needs `_io.FileIO`, unavailable on WASM)
- `scripts/find_stdlib_deps.py` — Traces transitive stdlib imports at file
  level; output goes to `crates/RustPython/Lib/freeze_allowlist.txt`

## Teaching Levels

The `--level` flag (CLI) or dropdown (web) controls which Python constructs are
allowed. The checker (`spython-core/src/checker.rs`) walks statements and
expressions, emitting diagnostics for forbidden constructs.

| Level | Name      | Adds                                                              |
| ----- | --------- | ----------------------------------------------------------------- |
| 1     | Functions | `def`, `if`/`elif`/`else`, `return`, scalars, string `[]`         |
| 2     | Types     | `class` (Enum / `@dataclass`), `match`                            |
| 3     | Arrays    | `list` literals, `for`, `while`, `+=`                             |
| 4     | Classes   | full `class` with methods, `dict`/`set`, comprehensions, `lambda` |
| 5     | Full      | unrestricted (only annotations still required)                    |

Default is level 1 (most restricted). Usage:

```bash
spython run --level 2 file.py
spython check --level 3 file.py
```

The `Level` enum lives in `spython-core/src/checker.rs` and is re-exported from
`spython-core/src/lib.rs`. The WASM `repl_new` export accepts a `level: u8`
parameter; the web UI sends it via the `load` message.

## Annotation Rules

Annotation lint rules in `spython-core/src/lints.rs` are checked at all levels:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function/method must have a return type annotation
- Every class-body variable assignment must have a type annotation
