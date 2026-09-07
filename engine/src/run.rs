//! Running Python: the interpreter every run starts, and the three commands
//! that reach it.

use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};

use crate::SpythonError;
use crate::ffi::register_ffi_module;

#[cfg(not(target_arch = "wasm32"))]
use crate::{Level, check, error::read_file};
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// The doctest runner Python source, which the `test` command and the REPL's
/// own `run_doctests` both drive.
pub const DOCTEST_RUNNER: &str = include_str!("doctest_runner.py");

/// Create a RustPython interpreter with the standard library loaded.
pub fn new_interpreter() -> vm::Interpreter {
    new_interpreter_with_argv(Vec::new())
}

/// The [`new_interpreter`] the `test` command needs, which hands the runner
/// the files to test in `sys.argv`.
fn new_interpreter_with_argv(argv: Vec<String>) -> vm::Interpreter {
    let mut settings = vm::Settings::default();
    settings.write_bytecode = false;
    settings.argv = argv;
    InterpreterBuilder::new()
        .settings(settings)
        .init_stdlib()
        .interpreter()
}

/// Run Python source in a fresh interpreter, with `dir` on `sys.path`.
pub fn run_source(source: &str, filename: &str, dir: &str) -> Result<(), SpythonError> {
    run_source_with_argv(source, filename, dir, Vec::new())
}

/// The [`run_source`] that also says what `sys.argv` holds.
fn run_source_with_argv(
    source: &str,
    filename: &str,
    dir: &str,
    argv: Vec<String>,
) -> Result<(), SpythonError> {
    let interp = new_interpreter_with_argv(argv);
    let source = source.to_owned();
    let filename = filename.to_owned();
    let dir = dir.to_owned();
    let code = interp.run(move |vm| {
        let scope = vm.new_scope_with_main()?;
        register_ffi_module(vm);
        vm.insert_sys_path(vm.new_pyobj(dir.as_str()))?;
        vm.run_string(scope, &source, &filename).map(drop)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(SpythonError::ProgramFailed)
    }
}

/// A file a run starts from, and everything running it takes.
#[cfg(not(target_arch = "wasm32"))]
pub struct Script {
    /// What the file holds.
    pub source: String,
    /// The name it runs under, which is the path as the user wrote it.
    pub name: String,
    /// The directory its imports resolve from.
    pub dir: String,
    /// The first-party files it imports, each a path relative to `dir` and
    /// its text.
    pub modules: Vec<(String, String)>,
}

/// Check `file` at `level` and read it.
///
/// Both a script and a session that starts from a file come through here, so
/// the two agree on what the file is and where it looks for its imports.
#[cfg(not(target_arch = "wasm32"))]
pub fn check_and_read(file: &Path, level: Level) -> Result<Script, SpythonError> {
    let modules = check::check_file(file, level)?;
    Ok(Script {
        source: read_file(file)?,
        name: file.to_string_lossy().into_owned(),
        dir: parent_dir(file),
        modules,
    })
}

/// The directory a file's imports resolve from, which is the one it sits in,
/// and the working directory for a bare name.
#[cfg(not(target_arch = "wasm32"))]
fn parent_dir(file: &Path) -> String {
    match file.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_string_lossy().into_owned(),
        _ => ".".to_owned(),
    }
}

/// Type-check a Python script and run it.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_main(file: &Path, level: Level) -> Result<(), SpythonError> {
    let script = check_and_read(file, level)?;
    run_source(&script.source, &script.name, &script.dir)
}

/// Run doctests for the given files.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_test(files: &[PathBuf], level: Level, verbose: bool) -> Result<(), SpythonError> {
    if files.is_empty() {
        return Ok(());
    }

    // A file that is not there is an error, the way it is for `run`. Silently
    // reporting no failures for a name the student mistyped is worse than
    // saying the name is wrong.
    for file in files {
        check::check_file(file, level)?;
    }

    let mut argv = vec![TEST_NAME.to_owned()];
    if verbose {
        argv.push("--verbose".to_owned());
    }
    argv.extend(files.iter().map(|file| file.to_string_lossy().into_owned()));

    let script = format!("{DOCTEST_RUNNER}\n{TEST_MAIN}");
    if run_source_with_argv(&script, TEST_NAME, ".", argv).is_err() {
        return Err(SpythonError::TestsFailed);
    }
    Ok(())
}

/// What drives the runner: the files come in through `sys.argv`, so a path
/// with a quote or a backslash in it needs no escaping here.
#[cfg(not(target_arch = "wasm32"))]
const TEST_MAIN: &str = "import sys\nsys.exit(_main(sys.argv[1:]))\n";

/// The name the test run reports itself under, as `sys.argv[0]` and as the
/// file a traceback in the driver points at.
#[cfg(not(target_arch = "wasm32"))]
const TEST_NAME: &str = "<test>";
