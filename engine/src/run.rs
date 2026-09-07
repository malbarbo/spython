//! Running Python: the interpreter every run starts, and the three commands
//! that reach it.

use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};

use crate::{SpythonError, register_ffi_module};

#[cfg(not(target_arch = "wasm32"))]
use crate::{Level, check, error::read_file};
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// The doctest runner Python source. Shared between the CLI `test` command
/// and the WASM REPL so the string is embedded only once per binary.
pub const DOCTEST_RUNNER: &str = include_str!("doctest_runner.py");

/// Create a RustPython interpreter with the standard library loaded.
pub fn new_interpreter() -> vm::Interpreter {
    let mut settings = vm::Settings::default();
    settings.write_bytecode = false;
    InterpreterBuilder::new()
        .settings(settings)
        .init_stdlib()
        .interpreter()
}

/// Run Python source in a fresh interpreter, with `dir` on `sys.path`.
pub fn run_source(source: &str, filename: &str, dir: &str) -> Result<(), SpythonError> {
    let interp = new_interpreter();
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

/// Type-check a Python script and run it.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_main(file: &Path, level: Level) -> Result<(), SpythonError> {
    check::check_file(file, level)?;

    let source = read_file(file)?;
    let file_str = file.to_string_lossy().into_owned();
    let parent_dir = file
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_owned();

    run_source(&source, &file_str, &parent_dir)
}

/// Type-check a Python file and stop there.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_check(file: &Path, level: Level) -> Result<(), SpythonError> {
    check::check_file(file, level).map(drop)
}

/// A path as a Python string literal's body.
#[cfg(not(target_arch = "wasm32"))]
fn escape(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
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

    let py_verbose = if verbose { "True" } else { "False" };
    let doctest_runner = DOCTEST_RUNNER;
    let mut script = format!(
        "{doctest_runner}\n\
         import types, sys\n\
         def _run(path, root):\n\
         \x20   sys.path.insert(0, root)\n\
         \x20   mod = types.ModuleType('__doctest__')\n\
         \x20   mod.__file__ = path\n\
         \x20   with open(path) as f:\n\
         \x20       exec(compile(f.read(), path, 'exec'), vars(mod))\n\
         \x20   return run_doctests(mod, verbose={py_verbose})\n\
         total = 0\n",
    );
    let print_names = files.len() > 1;
    for file in files {
        let path = file.to_string_lossy();
        let escaped = escape(&path);
        // The doctests import the way the file they came from does, so they
        // run with its own directory on the path -- the same directory the
        // static check resolved them against.
        let root = escape(&file.parent().unwrap_or(Path::new(".")).to_string_lossy());
        let root = if root.is_empty() {
            ".".to_owned()
        } else {
            root
        };
        if print_names {
            script.push_str(&format!("print(\"{escaped}\", flush=True)\n"));
        }
        script.push_str(&format!("total += _run(\"{escaped}\", \"{root}\")\n"));
    }
    script.push_str("sys.exit(total)\n");

    if run_source(&script, "<test>", ".").is_err() {
        return Err(SpythonError::TestsFailed);
    }
    Ok(())
}
