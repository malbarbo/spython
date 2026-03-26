# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## What is spython

`spython` is a Python interpreter with integrated type checking aimed at
students. It enforces complete type annotations before running code — useful for
teaching typed Python.

## Setup

```bash
git clone <repo-url>
```

Dependencies are fetched automatically via git dependencies in `Cargo.toml`
(no submodules).

## Build & Run

```bash
# Build
cargo build

# Build optimized
cargo build --release

# Run a Python script (with type checking)
cargo run -- run file.py

# Start REPL (no type checking)
cargo run
```

**After every modification**, run these commands and fix any failures before
considering the task done:

```bash
cargo clippy            # lint checks
cargo fmt -- --check    # formatting check
cargo test              # all tests
```

Manual testing is also done with `.py` files in the repo root (e.g. `a.py`,
`simple.py`, `x.py`, etc.).

## Architecture

The project is a Rust binary (`src/`) with one local crate:

- `spython-core` — shared library used by both the CLI and the WASM build

External dependencies (via git):

- `malbarbo/RustPython` (branch `spython-0.1`) — the Python interpreter
- `malbarbo/ruff` (branch `spython-0.1`) — provides the `ty` type checker and
  the `ruff_python_*` AST/parser crates

**Fork policy**: Minimize changes to the RustPython and ruff forks. Prefer
hooking into their public APIs from `spython-core` or `src/`. Changes to forks
are harder to track and complicate upstream updates.

Current fork customizations (RustPython, 3 commits on `spython-0.1`):

1. Fix WASM imports and interrupt — `FileIO`/`inspect` optional, `check_interrupt`
   FFI, `OsError` type fix
2. Allowlist-based freeze filtering and extra-modules feature — `FREEZE_SEEDS`
   env var for stdlib freeze filtering, `extra-modules` feature flag
3. Use ruff git dependency from `malbarbo/ruff` fork

Current fork customizations (ruff, 1 commit on `spython-0.1`):

1. Support `TYPESHED_ALLOWLIST` env var to trim typeshed stubs in zip

**Execution pipeline** (see `src/main.rs:run_checked`; level defaults to 0):

1. Resolve the given `.py` file to an absolute path and collect all transitively
   imported local Python files (`collect_import_files`).
2. Build a `ty` `ProjectDatabase` from those files (`build_db`).
3. Run spython's custom checker (`spython-core/src/checker.rs`) — checks
   annotations and forbidden constructs based on the teaching level.
4. If the checker passes, run ty's type checker (`db.check()`).
5. If type checking passes, execute the script with RustPython.

**Source files:**

- `src/main.rs` — CLI, pipeline orchestration, import resolution
- `src/repl.rs` — Interactive REPL with syntax highlighting, auto-indent,
  tab completion, and multi-line editing (uses rustyline directly)
- `spython-core/src/checker.rs` — AST walker: annotation checks + construct
  restrictions
- `spython-core/src/lints.rs` — Lint rule declarations using `declare_lint!`
- `spython-core/src/doctest_runner.py` — Minimal doctest runner (avoids stdlib
  `doctest` which needs `_io.FileIO`, unavailable on WASM)
- `scripts/find_stdlib_deps.py` — Traces transitive stdlib imports at file
  level (useful for verifying freeze seeds)

## Binary Size

Binary size matters — spython is distributed as a WASM binary for the web
interface. Only include stdlib modules and typeshed stubs that are actually
needed. Before adding a new dependency, stdlib seed, or typeshed entry, check
the impact on binary size. Avoid pulling in heavy modules (e.g. `os`, `re`,
`inspect`) unless strictly required.

## Freeze Seeds

The `FREEZE_SEEDS` env var (set in `.cargo/config.toml`) lists the Python
stdlib modules that spython needs at runtime. At build time, the
`rustpython-pylib` build script resolves transitive dependencies and generates
a freeze allowlist automatically. Only these modules are compiled into the
binary as frozen bytecode.

Current seeds: `dataclasses,encodings,enum,typing`

## Teaching Levels

The `--level` flag controls which Python constructs are allowed. The checker
(`spython-core/src/checker.rs`) walks statements and expressions, emitting
diagnostics for forbidden constructs.

| Level | Name       | Adds                                                              |
| ----- | ---------- | ----------------------------------------------------------------- |
| 0     | Functions  | `def`, `return`, scalars, string `[]`                             |
| 1     | Selection  | `if`/`elif`/`else`                                                |
| 2     | Types      | `class` (Enum / `@dataclass`), `match`                            |
| 3     | Repetition | `list` literals, `for`, `while`, `+=`                             |
| 4     | Classes    | full `class` with methods, `dict`/`set`, comprehensions, `lambda` |
| 5     | Full       | unrestricted (only annotations still required)                    |

Default is level 0 (most restricted). Usage:

```bash
spython run --level 2 file.py
spython check --level 3 file.py
```

The `Level` enum lives in `spython-core/src/checker.rs` and is re-exported from
`spython-core/src/lib.rs`. The WASM `repl_new` export accepts a `level: u8`
parameter.

## Annotation Rules

Annotation lint rules in `spython-core/src/lints.rs` are checked at all levels:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function/method must have a return type annotation
- Every class-body variable assignment must have a type annotation
