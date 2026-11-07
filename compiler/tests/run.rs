//! Each test compiles a program, runs it in wasmtime and in CPython, and
//! compares what the two print and how they exit.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use engine::Level;
use engine::check::check_source;

/// The module of `source`, which has to pass the checker at the last level.
fn compile(source: &str) -> Result<Vec<u8>, compiler::Error> {
    let checked = match check_source(source, Level::Full, false) {
        Ok(checked) => checked,
        Err(errors) => panic!("the checker rejects the program: {:?}", errors.diagnostics),
    };
    compiler::compile(&checked.db, checked.file)
}

/// The path of a file for the test `name`. Tests run at the same time, so
/// each one has its own.
fn path(name: &str, extension: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.{extension}"))
}

/// Runs `module` in the wasmtime on the `PATH`, or in the one that
/// `WASMTIME` names.
fn wasmtime(name: &str, module: &[u8]) -> Output {
    let path = path(name, "wasm");
    std::fs::write(&path, module).expect("the module written to disk");
    let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
    Command::new(wasmtime)
        .args(["run", "-W", "gc=y,function-references=y,exceptions=y"])
        .args(["--invoke", "_start"])
        .arg(&path)
        .output()
        .expect("wasmtime to start")
}

/// Runs `source` in the python3 on the `PATH`, or in the one that `PYTHON`
/// names.
fn cpython(name: &str, source: &str) -> Output {
    let path = path(name, "py");
    std::fs::write(&path, source).expect("the program written to disk");
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    Command::new(python)
        .arg(&path)
        .output()
        .expect("python3 to start")
}

/// Asserts that the compiled `source` prints what CPython prints, and exits
/// as CPython exits.
fn same_as_cpython(name: &str, source: &str) {
    let module = compile(source).expect("the program to compile");
    let ours = wasmtime(name, &module);
    let theirs = cpython(name, source);
    assert_eq!(
        String::from_utf8_lossy(&ours.stdout),
        String::from_utf8_lossy(&theirs.stdout),
        "stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(&ours.stderr),
        String::from_utf8_lossy(&theirs.stderr),
        "stderr"
    );
    assert_eq!(ours.status.code(), theirs.status.code(), "exit code");
}

fn unsupported(source: &str) -> String {
    match compile(source) {
        Err(compiler::Error::Unsupported { what, .. }) => what,
        Err(error) => panic!("an invalid module: {error:?}"),
        Ok(_) => panic!("the program compiled"),
    }
}

#[test]
fn an_empty_program_prints_nothing() {
    same_as_cpython("an_empty_program_prints_nothing", "");
}

#[test]
fn print_writes_a_str_and_a_newline() {
    same_as_cpython("print_writes_a_str_and_a_newline", "print('olá')\n");
}

#[test]
fn print_without_arguments_writes_a_newline() {
    same_as_cpython("print_without_arguments_writes_a_newline", "print()\n");
}

#[test]
fn print_separates_its_arguments_with_a_space() {
    same_as_cpython(
        "print_separates_its_arguments_with_a_space",
        "print('a', '', 'ção', '日本', '😀')\nprint('a')\n",
    );
}

#[test]
fn print_writes_a_str_longer_than_the_buffer() {
    let text: String = "ab€😀".repeat(20_000);
    same_as_cpython(
        "print_writes_a_str_longer_than_the_buffer",
        &format!("print('{text}', '{text}')\nprint('{text}')\n"),
    );
}

#[test]
fn a_str_expression_as_a_statement_prints_nothing() {
    same_as_cpython(
        "a_str_expression_as_a_statement_prints_nothing",
        "'nada'\nprint('nada')\n'nada'\n",
    );
}

#[test]
fn a_str_with_the_escape_of_a_surrogate_is_unsupported() {
    assert_eq!(
        unsupported("print('\\ud800')\n"),
        "a str with an escape that may be a surrogate"
    );
}

#[test]
fn print_with_a_keyword_argument_is_unsupported() {
    assert_eq!(
        unsupported("print('a', end='')\n"),
        "print with keyword arguments"
    );
}
