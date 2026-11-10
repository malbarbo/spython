//! A run, or the doctests, of a file through the compiler to WasmGC, with
//! the module run in the wasmtime on the `PATH`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use engine::check::{check_file_keeping_db, doctest_program};
use engine::{Level, SpythonError, read_file};

/// Checks `file` at `level`, compiles it, and runs the module.
pub fn run(file: &Path, level: Level) -> Result<(), SpythonError> {
    let checked = check_file_keeping_db(file, level)?;
    let name = file.to_string_lossy();
    let module = compiler::compile(&checked.db, checked.file, &name)
        .map_err(|error| compile_error(file, &error))?;
    if execute(&module)? {
        Ok(())
    } else {
        Err(SpythonError::ProgramFailed)
    }
}

/// Checks each of `files` at `level`, and runs its doctests in a module of
/// its own, as the `test` command of the interpreter does. With `verbose`,
/// the report has the examples that pass too.
pub fn test(files: &[PathBuf], level: Level, verbose: bool) -> Result<(), SpythonError> {
    // No file runs before the checker accepts every file.
    let checked = files
        .iter()
        .map(|file| check_file_keeping_db(file, level))
        .collect::<Result<Vec<_>, _>>()?;
    let mut passed = true;
    for (file, checked) in files.iter().zip(&checked) {
        let name = file.to_string_lossy();
        if files.len() > 1 {
            let mut stdout = std::io::stdout();
            writeln!(stdout, "{name}")
                .and_then(|()| stdout.flush())
                .map_err(|err| other(format!("cannot write the name of the file: {err}")))?;
        }
        let program = doctest_program(&checked.db, checked.file);
        let docstrings: Vec<compiler::Docstring> = program
            .docstrings
            .into_iter()
            .map(|docstring| compiler::Docstring {
                owner: docstring.owner,
                function: docstring.name,
                examples: docstring
                    .examples
                    .into_iter()
                    .map(|(line, example)| compiler::Example {
                        line,
                        source: example.source,
                        want: example.want,
                        exc_msg: example.exc_msg,
                    })
                    .collect(),
            })
            .collect();
        let module =
            compiler::compile_tests(&program.db, program.file, &name, &docstrings, verbose)
                .map_err(|error| compile_error(file, &error))?;
        passed &= execute(&module)?;
    }
    if passed {
        Ok(())
    } else {
        Err(SpythonError::TestsFailed)
    }
}

/// The error of a file that the compiler does not compile.
fn compile_error(file: &Path, error: &compiler::Error) -> SpythonError {
    let name = file.to_string_lossy();
    match error {
        compiler::Error::Unsupported { range, what } => {
            let place = read_file(file)
                .ok()
                .and_then(|source| line_and_column(&source, usize::from(range.start())));
            match place {
                Some((line, column)) => other(format!(
                    "{name}:{line}:{column}: the wasmgc backend does not support {what}"
                )),
                // The code of a doctest comes after the file for the
                // compiler.
                None => other(format!(
                    "{name}: the wasmgc backend does not support {what} in a doctest"
                )),
            }
        }
        compiler::Error::Invalid(err) => {
            other(format!("the compiler made an invalid module: {err}"))
        }
    }
}

/// Runs `module` in the wasmtime on the `PATH`. Returns `true` if the
/// program ended with 0, `false` otherwise.
fn execute(module: &compiler::Module) -> Result<bool, SpythonError> {
    let mut wasm = tempfile::Builder::new()
        .prefix("spython-")
        .suffix(".wasm")
        .tempfile()
        .map_err(|err| other(format!("cannot create the module file: {err}")))?;
    wasm.write_all(&module.bytes)
        .map_err(|err| other(format!("cannot write the module file: {err}")))?;

    // The stack of 8 MiB holds 1000 frames of the usual size, the recursion
    // limit of CPython, and a backtrace of 2000 frames holds them too. The
    // standard error waits for the end, because the error of a program that
    // wasmtime stopped becomes a traceback.
    let output = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y,max-wasm-stack=8388608",
            "-D",
            "max-backtrace=2000",
            "--invoke",
            "_start",
        ])
        .arg(wasm.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| other(format!("cannot run wasmtime: {err}")))?;
    // The module wrote the traceback of a program that failed, unless
    // wasmtime stopped it.
    let error = String::from_utf8_lossy(&output.stderr);
    let mut stderr = std::io::stderr();
    let traceback = module
        .memory_error(&error)
        .or_else(|| module.recursion_error(&error));
    let written = match traceback {
        Some(traceback) => stderr.write_all(traceback.as_bytes()),
        None => stderr.write_all(&output.stderr),
    };
    written.map_err(|err| other(format!("cannot write the error: {err}")))?;
    Ok(output.status.success())
}

/// The line and the column of `offset` in `source`, both from 1, where the
/// column counts code points. `None` when `source` has no such offset.
fn line_and_column(source: &str, offset: usize) -> Option<(usize, usize)> {
    let before = source.get(..offset)?;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line = before.matches('\n').count() + 1;
    let column = before.get(line_start..)?.chars().count() + 1;
    Some((line, column))
}

fn other(message: String) -> SpythonError {
    SpythonError::Other(message.into())
}
