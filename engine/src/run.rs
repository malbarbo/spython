//! Running Python: the interpreter every run starts, and the three commands
//! that reach it.

use std::collections::HashSet;
use std::sync::OnceLock;

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

/// Run the doctests of `source`, which is what the file `filename` holds.
///
/// The `test` command reads its own files, inside the runner. A page has no
/// files to read, so it hands over the text it already has, which travels in
/// `sys.argv` and so needs no escaping into the script that drives the runner.
pub fn run_test_source(source: &str, filename: &str) -> Result<(), SpythonError> {
    let script = format!("{DOCTEST_RUNNER}\n{TEST_SOURCE_MAIN}");
    let argv = vec![TEST_NAME.to_owned(), filename.to_owned(), source.to_owned()];
    run_source_with_argv(&script, TEST_NAME, ".", argv).map_err(|_| SpythonError::TestsFailed)
}

/// What drives the runner over a source that came in already read.
const TEST_SOURCE_MAIN: &str = "import sys\nsys.exit(_run_file(sys.argv[1], sys.argv[2]))\n";

/// The name a test run reports itself under, as `sys.argv[0]` and as the file
/// a traceback in the driver points at.
const TEST_NAME: &str = "<test>";

/// Keep `source` as the text of `filename`, which a traceback shows its lines
/// from. A page has no disk to read them from, and the file on a disk may
/// have changed since it ran.
pub(crate) fn keep_source(vm: &vm::VirtualMachine, filename: &str, source: &str) {
    vm.state
        .sources
        .lock()
        .insert(filename.to_owned(), source.to_owned());
}

/// Returns `true` if the interpreter imports `package` without looking at
/// `sys.path`, `false` otherwise. The interpreter looks first at what it
/// has built in, frozen, or already in `sys.modules`, so a file of that name
/// beside the program is never imported.
pub(crate) fn has_own_package(package: &str) -> bool {
    static PACKAGES: OnceLock<HashSet<String>> = OnceLock::new();
    PACKAGES
        .get_or_init(|| {
            new_interpreter().enter(|vm| {
                register_ffi_module(vm);
                let mut names: Vec<String> = vm
                    .state
                    .module_defs
                    .keys()
                    .chain(vm.state.frozen.keys())
                    .map(|name| (*name).to_owned())
                    .collect();
                if let Ok(modules) = vm.sys_module.get_attr("modules", vm)
                    && let Ok(keys) = vm.call_method(&modules, "keys", ())
                    && let Ok(keys) = keys.try_to_value::<Vec<String>>(vm)
                {
                    names.extend(keys);
                }
                names
                    .iter()
                    .map(|name| name.split('.').next().unwrap_or(name).to_owned())
                    .collect()
            })
        })
        .contains(package)
}

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
    let builder = InterpreterBuilder::new().settings(settings).init_stdlib();
    let scene = crate::scene::module_def(&builder.ctx);
    let builder = builder.add_native_module(scene);
    #[cfg(not(target_arch = "wasm32"))]
    let builder = {
        let display = crate::display::module_def(&builder.ctx);
        builder.add_native_module(display)
    };
    builder.interpreter()
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
        keep_source(vm, &filename, &source);
        vm.run_string(scope, &source, &filename).map(drop)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(SpythonError::ProgramFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interpreter_has_its_own_builtin_frozen_and_registered_packages() {
        for package in [
            "sys",
            "math",
            "copy",
            "enum",
            "typing",
            "spython",
            "_spython_ffi",
        ] {
            assert!(has_own_package(package), "{package}");
        }
        for package in ["re", "helper", "spython_helpers"] {
            assert!(!has_own_package(package), "{package}");
        }
    }

    /// The tests in `doctest_runner_test.py` run after the runner, in the
    /// interpreter that runs it for real. A failed assert fails the run.
    #[test]
    fn the_doctest_runner_passes_its_tests() {
        let script = format!(
            "{DOCTEST_RUNNER}\n{}",
            include_str!("doctest_runner_test.py")
        );
        assert!(run_source(&script, "doctest_runner_test.py", ".").is_ok());
    }
}
