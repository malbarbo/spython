//! Running Python: the interpreter every run starts, and the three commands
//! that reach it.

use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};

use crate::SpythonError;
use crate::ffi::register_ffi_module;

/// What a page cannot do, which is everything that starts from a file on
/// disk. Only the command line gets this far.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{Script, check_and_read, run_main, run_test};

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
