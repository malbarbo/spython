# Release notes

The notes for the next release. The ones for the releases already out are on
the [releases page](https://github.com/malbarbo/spython/releases).

## spython 0.4.0

spython is a version of [Python](https://www.python.org/) for students. It
checks types before running anything, restricts the language by level, and
comes with a REPL and libraries for drawing images and writing interactive
programs.

Try it online: <https://malbarbo.pro.br/spython/>

### Command line

- `spython test files` runs the doctests. It is what `spython check` used to
  do, under the name the other tools use for it.
- `spython check file` is new. It checks the types and stops there, saying
  nothing when the file is right and printing the diagnostics when it is not.
- A file that is not there is an error for `test`, the way it already was for
  `run`. A mistyped name is no longer read as a file with no tests in it.
- `spython format --check paths` names the files that are not formatted and
  changes none of them, for a continuous integration job to run.
- `spython format` with no path formats what it reads on the standard input
  onto the standard output, for an editor to call.
- `spython file.py` runs the file, and `spython -l 2` starts a level 2 REPL,
  without naming the command.

### REPL

- The welcome message says which level the session is on: `Welcome to spython
  0.4.0 (...), level 0 - Functions.`
- Tab and Enter indent the same way, reading the tokens instead of a trailing
  colon, and Backspace moves by the same step. An input with a bracket left
  open now indents on Tab.
- Tab on a keyword offers the space that opens what follows it.
- `:type` ends in a failing status when it cannot infer the type, which the
  web version shows as an error.

### Checks

- Three new lints: a function or class defined twice in the same block
  (levels 0 to 3), a bare `list` where the annotation needs its type
  parameter, and a local whose type cannot be worked out.
- A `bool` operand whose type is `Any` no longer trips the check on `bool` in
  arithmetic.
- Diagnostics come out in the same order on every platform.

### Other

- The `World` loop draws after every callback that changes the state.
- `test` says which file it is on before running its examples.

### Supported platforms

- Linux x86_64 / aarch64 (musl)
- macOS x86_64 / aarch64
- Windows x86_64
- WASM (wasm32-wasip1)

Powered by RustPython 0.5.0 and ty/ruff 0.16.4.

**Full changelog**: https://github.com/malbarbo/spython/compare/v0.3.0...v0.4.0
