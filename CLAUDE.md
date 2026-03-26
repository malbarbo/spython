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
cargo run -p cli -- run file.py

# Start REPL (no type checking)
cargo run -p cli
```

**After every modification**, run these commands and fix any failures before
considering the task done:

```bash
cargo clippy --workspace    # lint checks
cargo fmt -- --check        # formatting check
cargo test --workspace      # all tests
```

Manual testing is also done with `.py` files in the repo root (e.g. `a.py`,
`simple.py`, `x.py`, etc.).

### WASM Build

The CLI crate cannot be compiled for WASM. To build the WASM binary:

```bash
cargo build -p wasm --target wasm32-wasip1 --release
```

### WASM Exports

The `wasm` crate exposes a C FFI for the web REPL. Key exports:

- `repl_new(source, len, level)` → `*mut ReplState`
- `repl_run(repl, code, len)` → `u32` (OK/ERROR/QUIT)
- `repl_complete(repl, text, len, cursor_pos)` → `*mut c_char` or null
- `repl_destroy(repl)`
- `format(source, len)` → `*mut c_char`
- `version()` → `*mut c_char`

`repl_complete` returns a space-separated string: `"c <startpos> <candidates...>"`
for completions, `"i <spaces>"` for indentation, or null for no action.

### Developing the Forks

Local clones for development:

- RustPython: `~/projetos/RustPython` (branch `spython-0.1`)
- ruff: `~/projetos/ruff` (branch `spython-0.1`)

After pushing changes to a fork, run `cargo update -p <crate>` in this repo
to pick up the new commit.

### Troubleshooting

If the REPL fails with `ImportError: No such frozen object named
_frozen_importlib`, the frozen stdlib cache is stale. Fix with:

```bash
cargo clean -p rustpython-pylib
```

## Architecture

The workspace has three crates:

- `engine` — shared library used by both the CLI and the WASM build
- `cli` — CLI binary (produces the `spython` executable)
- `wasm` — WASM shim (thin FFI layer over `engine`)

External dependencies (via git):

- `malbarbo/RustPython` (branch `spython-0.1`) — the Python interpreter
- `malbarbo/ruff` (branch `spython-0.1`) — provides the `ty` type checker and
  the `ruff_python_*` AST/parser crates

**Fork policy**: Minimize changes to the RustPython and ruff forks. Prefer
hooking into their public APIs from `engine` or `cli`. Changes to forks are
harder to track and complicate upstream updates.

Current fork customizations (RustPython, 3 commits on `spython-0.1`):

1. Fix WASM imports and interrupt — `FileIO`/`inspect` optional, `check_interrupt`
   FFI, `OsError` type fix
2. Allowlist-based freeze filtering and extra-modules feature — `FREEZE_SEEDS`
   env var for stdlib freeze filtering, `extra-modules` feature flag
3. Use ruff git dependency from `malbarbo/ruff` fork

Current fork customizations (ruff, 1 commit on `spython-0.1`):

1. Support `TYPESHED_ALLOWLIST` env var to trim typeshed stubs in zip

**Execution pipeline** (see `cli/main.rs:run_checked`; level defaults to 0):

1. Resolve the given `.py` file to an absolute path and collect all transitively
   imported local Python files (`collect_import_files`).
2. Build a `ty` `ProjectDatabase` from those files (`build_db`).
3. Run spython's custom checker (`engine/src/checker.rs`) — checks
   annotations and forbidden constructs based on the teaching level.
4. If the checker passes, run ty's type checker (`db.check()`).
5. If type checking passes, execute the script with RustPython.

**Source files:**

- `cli/main.rs` — CLI, pipeline orchestration, import resolution
- `cli/repl.rs` — Interactive REPL with syntax highlighting, auto-indent,
  and multi-line editing (uses rustyline). Delegates completion to `engine`.
- `engine/src/completion.rs` — Tab completion logic shared by CLI and WASM:
  identifier/attribute/keyword completion and indent-vs-complete decision.
- `engine/src/checker.rs` — AST walker: annotation checks + construct
  restrictions
- `engine/src/lints.rs` — Lint rule declarations using `declare_lint!`
- `engine/src/doctest_runner.py` — Minimal doctest runner (avoids stdlib
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
(`engine/src/checker.rs`) walks statements and expressions, emitting
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

The `Level` enum lives in `engine/src/checker.rs` and is re-exported from
`engine/src/lib.rs`. The WASM `repl_new` export accepts a `level: u8`
parameter.

## Annotation Rules

Annotation lint rules in `engine/src/lints.rs` are checked at all levels:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function/method must have a return type annotation
- Every class-body variable assignment must have a type annotation
