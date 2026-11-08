//! A run through the compiler to WasmGC, with the module run in the
//! wasmtime on the `PATH`.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use engine::check::check_file_keeping_db;
use engine::{Level, SpythonError, read_file};

/// Checks `file` at `level`, compiles it, and runs the module.
pub fn run(file: &Path, level: Level) -> Result<(), SpythonError> {
    let checked = check_file_keeping_db(file, level)?;
    let name = file.to_string_lossy();
    let module = match compiler::compile(&checked.db, checked.file, &name) {
        Ok(module) => module,
        Err(compiler::Error::Unsupported { range, what }) => {
            let source = read_file(file)?;
            let (line, column) = line_and_column(&source, usize::from(range.start()));
            return Err(other(format!(
                "{name}:{line}:{column}: the wasmgc backend does not support {what}"
            )));
        }
        Err(compiler::Error::Invalid(err)) => {
            return Err(other(format!("the compiler made an invalid module: {err}")));
        }
    };

    let mut wasm = tempfile::Builder::new()
        .prefix("spython-")
        .suffix(".wasm")
        .tempfile()
        .map_err(|err| other(format!("cannot create the module file: {err}")))?;
    wasm.write_all(&module.bytes)
        .map_err(|err| other(format!("cannot write the module file: {err}")))?;

    // The stack of 8 MiB holds 1000 frames of the usual size, the recursion
    // limit of CPython, and a backtrace of 2000 frames holds them too. The
    // module writes to the standard error only before it ends, so the error
    // comes after the standard output when it waits for the end.
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
    if output.status.success() {
        return Ok(());
    }
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
    Err(SpythonError::ProgramFailed)
}

/// The line and the column of `offset` in `source`, both from 1, where the
/// column counts code points.
fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line = before.matches('\n').count() + 1;
    let column = before[line_start..].chars().count() + 1;
    (line, column)
}

fn other(message: String) -> SpythonError {
    SpythonError::Other(message.into())
}
