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

# Run a Python script — shorthand (a bare FILE implies `run`)
cargo run -p cli -- file.py
cargo run -p cli -- file.py --level 2

# Start REPL (no type checking)
cargo run -p cli
```

When invoked with no subcommand, a leading positional `FILE` is dispatched to
`run` (so `spython file.py` == `spython run file.py`); with no file the REPL
starts, and any options are routed to it (so `spython -l 2` == `spython repl
--level 2`). The dispatch is a thin argv rewrite in `cli/main.rs`
(`effective_args`) that injects `run` before a bare file or `repl` before
bare options, leaving explicit subcommands, `--help`, and `--version`
untouched. A file literally named after a subcommand (e.g. `run` with no
extension) is still read as that subcommand unless escaped with `--`
(`spython -- run`).

**After every modification**, run these commands and fix any failures before
considering the task done:

```bash
make check                  # clippy + fmt + ruff format + deno fmt
cargo test --workspace      # all tests
```

Manual testing is also done with `.py` files in the repo root (e.g. `a.py`,
`simple.py`, `x.py`, etc.).

CI runs on GitHub Actions (`.github/workflows/ci.yaml`): lint on Linux,
WASM build+test, and tests on 5 targets (Linux musl x86/arm, Windows,
macOS x86/arm). A separate release workflow (`.github/workflows/release.yaml`)
builds binaries and creates a GitHub release when a tag is pushed.

### WASM Build

The `wasm` crate is excluded from the default workspace. It is NOT compiled
by `cargo build`, `cargo test`, or `cargo clippy` (only `engine` and `cli`
are). To build the WASM binary:

```bash
make wasm                   # build + optimize
make test-wasm              # build + run Deno tests
```

WASM tests use Deno (`wasm/tests/`) and test the WASM binary directly via
`repl_new`, `repl_run`, `repl_complete`, `repl_ready`, `format`, and `version`
exports.
The test WASI polyfill (`wasm/tests/wasi.ts`) and env mock provide the
required WASM imports.

### WASM Exports

The `wasm` crate implements the slangs engine WASM protocol
(see `ENGINE.md` in the slangs repo). Key exports:

- `repl_new(code, len, config, config_len)` → `*mut ReplState`
- `repl_run(repl, code, len)` → `u32` (OK/ERROR/QUIT)
- `repl_complete(repl, text, len, cursor_pos)` → `*mut c_char` or null
- `repl_ready(repl, code, len)` → `i32` (-1 to run the input, otherwise the
  indentation the next line starts with)
- `repl_destroy(repl)`
- `format(source, len)` → `*mut c_char`
- `version()` → `*mut c_char`

`repl_complete` returns `"c <startpos>"` followed by one candidate per line
for completions, `"i <spaces>"` for indentation, or null for no action. The
candidates are separated by newlines, not spaces, because a candidate can end
in a space -- the way `import ` opens the next word.

`repl_ready` reads the input text only — the session pointer is ignored — and
answers `-1` for input no further line could finish as well, so the user gets
the error instead of a prompt that will not let go. It and the terminal REPL
both call `engine::parser::ready_state`, so the two prompts read a line the
same way.

`repl_new` config is a space-separated `key=value` string (compatible with the
slangs engine protocol). Currently supported: `level=<0-5>`. Unknown keys are
ignored.

### Developing the Forks

Local clones for development:

- RustPython: `~/projetos/RustPython` (branch `spython-0.3`)
- ruff: `~/projetos/ruff` (branch `spython-0.3`)

The branch name tracks the spython version it was rebased for; `spython-0.1`
is the previous generation (RustPython 0.5.0 / ruff 0.15.6) and is kept for
reference.

After pushing changes to a fork, run `cargo update -p <crate>` in this repo
to pick up the new commit.

### Version Strings

Version strings and build metadata (date, git hash) are centralized in the
`engine` crate. `engine/build.rs` captures the build date and short git hash
at compile time, and `engine/src/lib.rs` exposes `VERSION`, `LIBS_VERSION`,
`BUILD_DATE`, `GIT_HASH`, and `LONG_VERSION` as public constants. Both the CLI
and WASM crate import these constants. When updating the RustPython or ruff
dependency versions, update the `LIBS_VERSION` value in `engine/build.rs`.

### User Configuration

The CLI saves user preferences (level and theme) to `config_dir()/spython/config`
(e.g., `~/.config/spython/config` on Linux). The file uses `key=value` format.
The `--level` CLI flag overrides the saved level. See `cli/config.rs`.

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

- `malbarbo/RustPython` (branch `spython-0.3`, on upstream `cc1e55e3e4`) —
  the Python interpreter
- `malbarbo/ruff` (branch `spython-0.3`, on tag `0.16.4`) — provides the `ty`
  type checker and the `ruff_python_*` AST/parser crates

RustPython itself depends on a *different* ruff (`RustPython/ruff`, tag
`0.15.19-rustpython`, packages renamed `rustpython-ruff_*`), so two ruff
parser/AST trees are compiled. That is deliberate: porting RustPython's AST
patch into our fork was measured and shrinks the WASM binary by only 109 KB
(0.6%) while costing 65 patched files to carry across every upstream update.

**Fork policy**: Minimize changes to the RustPython and ruff forks. Prefer
hooking into their public APIs from `engine` or `cli`. Changes to forks are
harder to track and complicate upstream updates.

Current fork customizations (RustPython, 9 commits on `spython-0.3`):

1. Fix WASM imports and interrupt — `FileIO`/`inspect` optional, `check_interrupt`
   FFI, `OsError` type fix
2. Allowlist-based freeze filtering and extra-modules feature — `FREEZE_SEEDS`
   env var for stdlib freeze filtering, `extra-modules` feature flag
3. Add `host_env_extras` feature for fine-grained stdlib module control
4. Support `FREEZE_EXTRA_DIR` for out-of-tree Python modules
5. Gate `msvcrt` usage in `_io.rs` behind `host_env_extras` feature — fixes
   Windows build when `host_env_extras` is not enabled
6. Check interrupt in `eval_breaker_tripped` on WASM
7. Gate `winreg` `import_builtin` behind `host_env_extras`
8. Replace malachite with num-bigint/num-rational directly
9. Fix `compile_dir_filtered` to take parent as `&str` (upstream signature
   change)

The musl stack-bounds patch is gone: upstream now handles musl itself
(`NATIVE_RECURSION_LIMIT_UNMEASURED`).

Current fork customizations (ruff, 2 commits on `spython-0.3`):

1. Support `TYPESHED_ALLOWLIST` env var to trim typeshed stubs in zip
2. Remove zstd compression, use deflate with max level

**Execution pipeline** (see `cli/main.rs:run_checked`; level defaults to 0):

1. Resolve the given `.py` file to an absolute path and build a `ty`
   `ProjectDatabase` rooted at the file's **own directory** (`build_db`) --
   not the process's, so that `import helper` resolves beside the file, which
   is also where the interpreter looks (`sys.path[0]`).
2. From there, collect all transitively imported local Python files
   (`collect_import_files`), and restrict what gets reported to them.
3. Run spython's custom checker (`engine/src/checker.rs`) — checks
   annotations and forbidden constructs based on the teaching level.
4. If the checker passes, run ty's type checker (`db.check()`).
5. If type checking passes, execute the script with RustPython.

**Source files:**

- `engine/build.rs` — Captures build date and git hash at compile time
- `cli/main.rs` — CLI, pipeline orchestration, import resolution
- `cli/repl.rs` — The terminal's line editor: syntax highlighting,
  auto-indent, and multi-line editing (uses rustyline), plus the commands
  only a terminal answers (`:level`, `:theme`). Everything else about an
  input it hands to `engine/src/repl.rs`.
- `engine/src/repl.rs` — One REPL input at a time, in the state the previous
  ones left. `ReplSession` holds what does not need an interpreter (the
  accumulated source, the level, the modules the session started from) and
  owns the one table of `:` commands both frontends answer from; `ReplState`
  adds the interpreter, for the page, which has no line editor of its own.
- `engine/src/completion.rs` — Tab completion logic shared by CLI and WASM:
  identifier/attribute/keyword completion and the indent-vs-complete decision
  (how far to indent comes from `parser.rs`).
- `engine/src/parser.rs` — What the prompt does with the input as it stands
  (`ready_state`): whether it is finished, read from the ruff parser's errors
  and tokens, and how far in the next line starts (`continuation_indent`,
  which Tab reaches through `completion.rs`).
- `engine/src/checker.rs` — AST walker: annotation checks + construct
  restrictions
- `engine/src/lints.rs` — Lint rule declarations using `declare_lint!`
- `engine/src/lib.rs` — The checking entry points, over `in_memory_project`:
  one in-memory filesystem holding the source as `/user.py`, the spython
  library, and the first-party modules it may import under `/modules`. The
  REPL checks its inputs there, and so does the doctest check, which writes
  a synthetic source through the same door.
- `engine/src/doctests.rs` — Finds doctests in docstrings, reports malformed
  `>>>` / `...` prompts, and builds the synthetic source that lets the
  snippets be type-checked and the diagnostics mapped back to the docstring.
- `engine/src/spython_lib.rs` — The embedded copy of `lib/spython/*.py` that
  ty resolves against (runtime uses the frozen bytecode instead)
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

Current seeds: `dataclasses,encodings,enum,typing,spython`

## FREEZE_EXTRA_DIR

The `FREEZE_EXTRA_DIR` env var (set in `.cargo/config.toml`) points to an
additional directory searched for Python modules during freeze. This allows
the spython image library to live in this repo (`lib/spython/`) instead of
in the RustPython fork. Both the build script (dependency resolution) and
the `py_freeze!` proc macro (bytecode compilation) use this directory as a
fallback after the main `Lib/`.

## Teaching Levels

The `--level` flag controls which Python constructs are allowed. The checker
(`engine/src/checker.rs`) walks statements and expressions, emitting
diagnostics for forbidden constructs.

| Level | Name       | Adds                                                              |
| ----- | ---------- | ----------------------------------------------------------------- |
| 0     | Functions  | `def`, `return`, scalars, string `[]`                             |
| 1     | Selection  | `if`/`elif`/`else`                                                |
| 2     | User types | `class` (Enum / `@dataclass`), `match`                            |
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

### Beginner-level restrictions (0–3)

Independent of which constructs are allowed at each level, levels 0 through
3 apply additional lints aimed at common beginner mistakes. All are lifted
at level 4:

- `non-boolean-condition` — tests of `if`/`elif`/`while`/ternary/`assert`
  and operands of `and`/`or`/`not` must have type `bool` (checked via ty's
  inferred types, not just the AST).
- `bool-in-arithmetic` — `bool` is not allowed as an operand of `+`, `-`,
  `*`, `/`, `//`, `%`, `**`, augmented assignment with those operators, or
  unary `+`/`-`.
- `chained-comparison` — `Compare` with more than one comparator
  (`a < b < c`, `a == b != c`) is rejected.
- `bare-expression` — `Stmt::Expr` whose value isn't a `Call`,
  `StringLiteral` (docstring), or `EllipsisLiteral` is rejected.
  Suppressed in REPL context (`in_repl=true` in `check_file_annotations`)
  since a bare expression at a REPL prompt is the display-result idiom.
- `forbidden-default-arg` — function parameters cannot have default values.

## Annotation Rules

Annotation lint rules in `engine/src/lints.rs` are checked at all levels:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function/method must have a return type annotation
- Every class-body variable assignment must have a type annotation

## Image Library

The spython image library lives in `lib/spython/` and is frozen into the
binary via `FREEZE_EXTRA_DIR`. It provides SVG-based image generation for
teaching graphics programming. Modules:

- `color.py` — Color type and 140+ CSS named colors
- `style.py` — `fill()` and `stroke()` with keyword arguments
- `font.py` — Font type with family/size/style/weight/underline
- `image.py` — Path-based SVG rendering, shapes, transformations, composition;
  re-exports color, style, and font so users import everything from `spython.image`
- `system.py` — FFI bridge (`show_svg`, `get_key_event`, text measurement)
- `world.py` — Interactive animation (`World` class, `animate`)

All SVG float values are rounded to 6 decimal places (via `_f()` in
`image.py` and `style.py`) to ensure cross-platform reproducibility
(glibc, musl, macOS, Windows have different libm precision).

Python files are formatted with `ruff format` (checked by `make check`).
Image tests are in `cli/tests/images/` with SVG snapshots in
`cli/tests/snapshots/`.
