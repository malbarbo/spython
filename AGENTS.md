# spython

A version of Python for students: a type checker that runs before the code,
levels that restrict the language, a REPL, and libraries for images and
interactive programs. `engine/` checks a file, runs it on RustPython, and
answers the REPL. `cli/` is the binary. `wasm/` exports the engine for the
browser, and `wasm/tests/` runs it under Deno. `lib/spython/` is the Python
library that ships in the binary. `docs/` is the user guide.

The interpreter is a fork of RustPython and the checker is `ty`, from a fork
of ruff. Both come from `malbarbo/` on GitHub, on the branch named after the
spython version they were rebased for, `spython-0.4` today. Prefer a change
in `engine` or `cli` to a change in a fork. After you push to a fork, run
`cargo update -p <crate>` here to take the new commit. The two forks and the
`LIBS_VERSION` in `engine/build.rs` change together.

## Build and test

```sh
make check        # clippy, cargo fmt, ruff format, deno fmt
make test-rs      # cargo test --workspace
make test         # the tests, the deno tests and the image snapshots
make wasm         # the WASM binary
make test-wasm    # the deno tests on the WASM binary
cargo test -p spython --test integration   # snapshots of what the binary prints
cargo test --workspace -- --ignored        # image snapshots, slow
```

`cli/tests/integration.rs` has one test per file under `cli/tests/inputs`
and `cli/tests/images`. `cli/build.rs` writes the list, so adding a file is
enough.

The `wasm` crate is outside the default workspace. `cargo build`, `cargo
test`, and `cargo clippy` do not touch it. The WASM exports follow the
simplecode engine protocol, in `ENGINE.md` of the simplecode repository.
`repl_new` takes a config of `key=value` pairs separated by spaces, and
`level=<0-5>` is the one key today.

Run `make check` and `make test-rs` before you call a change done. Do not
leave test files in the root of the repository.

If the REPL fails with `ImportError: No such frozen object named
_frozen_importlib`, the frozen stdlib cache is stale. Run `cargo clean -p
rustpython-pylib`.

## Scope

Change what was asked and nothing else. A problem you see on the way is
something to report, not to fix in the same change. Before you start, write
the subject of the commit. If it does not fit in one concrete line, the
change is more than one, and it becomes more than one commit.

## Conventions in the code

- A bare file on the command line means `run`, and bare options mean `repl`.
  `cli()` in `cli/src/main.rs` reads the three as alternatives.
- The CLI saves the theme in `config_dir()/spython/config`, as a
  `key=value` line. The level comes from `--level` alone, and is 5 without
  it. See `cli/src/config.rs`.
- A program can import `dataclasses`, `enum`, `math`, `typing`, `spython`,
  and the modules beside it. `AVAILABLE_MODULES` in `engine/src/checker.rs`
  holds the four from the standard library, and the `unavailable-module`
  lint rejects the rest. The freeze filter and `typeshed_allowlist.txt` let
  through more, which the interpreter and the stubs need for themselves.
- Colour is decided by `color_stderr` and `color_stdout` in
  `engine/src/error.rs`, and nowhere else. A non-empty `NO_COLOR` keeps the
  escape codes out of both streams, as <https://no-color.org> asks, and a
  non-empty `FORCE_COLOR` asks for them where no terminal is watching. The
  page passes what it wants by hand.

# Writing

These rules cover every piece of English in the repository: comments, doc
comments, commit messages, and the docs.

## Comments

Delete first. Most comments should not exist. The name, the type, and the line
below already say it. Ask what the comment adds before you ask how it reads.

- Say why, not what. The reason for an assert, the case a branch guards, the
  invariant a caller has to keep.
- If the reader can see it in the line below, drop the comment.
- Never comment history. What the code used to do, a bug that was fixed, a
  function that is gone, none of that belongs in a comment about the code as
  it stands.
- Put the comment on the thing it describes. `// from the gleam project`
  belongs on the function that came from there, not at the top of the file.
- Check that the comment is true in every case the code covers. "only
  Windows reaches this" read well and was false.
- Keep it short. One or two lines is the norm. A paragraph needs a reason.
- A function that answers yes or no says so first: `Returns \`true\` if ...,
  \`false\` otherwise.` The details come after.

## How a sentence reads

Plain, active English. Subject, verb, object. The reader takes it in one word
at a time, left to right, without going back.

Avoid the constructions that make the reader do the grammar:

- **Passive voice.** It hides who acts.
  - No: `Nothing else is filtered.`
  - Yes: `Everything else stays.`
- **A preposition left at the end of a relative clause.** The object of the
  preposition sits at the head of the clause, so the reader only finds out at
  the end what to fill the gap with. Portuguese has no such construction,
  which makes it doubly hard to read.
  - No: `The path the compiled JavaScript names a source file by`
  - Yes: `The path the compiled JavaScript uses for a source file`
- **An elided verb.** Write the verb out.
  - No: `so only a relative path of plain names does`
  - Yes: `so the path has to be relative and made of plain names`
- **A pronoun for a noun that is far away.** Name the noun again.
  - No: `the head of the ones with a body`
  - Yes: `the head of a definition that has a body`
- **A verb and preposition that do not go together.** Use the ordinary one.
  - No: `What the definition takes of the input`
  - Yes: `How much of the input the definition covers`
- **A clause that ends in its verb.** The reader holds the noun until the
  verb arrives, which is the same trouble as a stranded preposition.
  - No: `A JavaScript context, and the runtime it runs in`
  - Yes: `A JavaScript context together with its runtime`
  - No: `the origin an svg text element takes`
  - Yes: `the origin of an svg text element`
- **A colon in the middle of a sentence.** It joins two halves and asks the
  reader to hold the first one. Write two sentences. A colon before a list
  or after a label is fine.
  - No: `One flag for the whole process: whatever runs stops at the next check.`
  - Yes: `One flag serves the whole process. Whatever runs stops at the next
    check.`
- **A dash in the middle of a sentence.** Same trouble as the colon. Write
  two sentences, or use a comma.
- **A metaphor for something the code states plainly.**
  - No: `a name that climbs out`, `however far the chain goes`
  - Yes: `a name with ..`, `directly or not`
- **A word that sounds clever.** `owe`, `pay for`, `travel`, `land`, `reach
  out`. Say `cost`, `go`, `is`, `call`.

## Commit messages

One change per commit. A refactor and the fix it prepares are two commits.
`engine: say each thing once`, which touched a macro, a cursor, the import
names, and the file list of a compile, should have been five.

The subject says what the change does to the code, in one line:

- A prefix for the part of the tree, then a lowercase sentence: `engine:`,
  `cli:`, `repl:`, `wasm:`, `lib:`, `docs:`, `ci:`, and `all:` when it
  crosses them.
- Imperative or present, active voice. `cli: say that a directory is not a
  module`.
- Name the concrete thing that changed and what happened to it. A function,
  a command, a flag, a message. The reader should know what to look for in
  the diff.
  - No: `all: ask the shell about the cursor, not just the text`
  - Yes: `all: change repl_ready to receive the cursor position`
  - No: `engine: tell the interrupt by the mark quickjs puts on it`
  - Yes: `engine: use JS_IsUncatchableError to detect an interrupt`
- No rhetorical contrast, no metaphor, no joke. `not just`, `instead of`, and
  `rather than` in a subject usually mean the subject is describing the old
  code instead of the new one.
- Under about 65 characters. No period at the end.

The body is optional. Write one when the subject leaves something out, which
is usually why:

- What was wrong, then what the change does. Two to four sentences, one
  paragraph, wrapped at 72 columns.
- No bullet lists, no list of files, no narration of the process (`also`,
  `while at it`, `cleaned up`). What is not worth a sentence is in the diff.
- No trailers. The message ends at the last line of the body. No
  `Co-Authored-By`, no `Signed-off-by`.

The subject of a real commit, with the body as it should read:

```
cli: check a file where the interpreter will look for its imports

The checker resolved an import from the current directory, and the
interpreter from the directory of the file, so a file run from elsewhere
passed the check and failed at run time. Both use the directory of the
file now.
```

## Docs, the README, and release notes

Prose, not lists of features. Show a thing where it is used, once. No counts
of modules or colors. No bold label followed by a dash.

Release notes open with the first sentence of the README, then the link to
the web version, then the list of changes. Each change is the subject of a
commit, or the subjects of a few commits joined, and not a rewrite of them.
