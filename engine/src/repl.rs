//! One REPL input at a time, in the state the previous ones left.
//!
//! `ReplSession` is the part that does not need to own the interpreter: the
//! accumulated source, the level, and what to do with an input. `ReplState`
//! adds the interpreter and the scope for the page, which has no line editor
//! of its own; the CLI keeps its own interpreter and drives a `ReplSession`
//! from inside rustyline.

use crate::Level;
use crate::{infer_expression_type, new_interpreter, register_ffi_module, type_check_repl_input};
use rustpython::vm;
use rustpython::vm::AsObject;
#[cfg(test)]
use std::sync::atomic::Ordering;

pub struct ReplState {
    // `scope` is wrapped in `Option` so the `Drop` impl can take it and
    // drop it inside the VM context (see below).
    scope: Option<vm::scope::Scope>,
    interp: Option<vm::Interpreter>,
    /// Accumulated source of all successfully type-checked and executed inputs.
    accumulated_source: String,
    /// Teaching level for annotation/construct checking.
    level: Level,
}

impl ReplState {
    /// Run a closure with access to the VM and the session's globals.
    pub fn with_vm<R>(
        &self,
        f: impl FnOnce(&vm::VirtualMachine, &vm::builtins::PyDictRef) -> R,
    ) -> R {
        let globals = &self.scope.as_ref().expect("scope is live").globals;
        self.interp
            .as_ref()
            .expect("interpreter is live")
            .enter(|vm| f(vm, globals))
    }

    /// The accumulated source of all successfully type-checked and executed inputs.
    pub fn accumulated_source(&self) -> &str {
        &self.accumulated_source
    }

    /// The teaching level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Append successfully executed source to the accumulator.
    pub fn append_source(&mut self, source: &str) {
        if !self.accumulated_source.is_empty() {
            self.accumulated_source.push('\n');
        }
        self.accumulated_source.push_str(source);
    }

    /// Run the REPL loop inside the VM context.
    ///
    /// The closure receives the VM and the scope. This is needed by the CLI
    /// REPL to create the rustyline `Editor` with a `ReplHelper` that borrows
    /// the VM.
    pub fn enter<R>(&self, f: impl FnOnce(&vm::VirtualMachine, vm::scope::Scope) -> R) -> R {
        let scope = self.scope.as_ref().expect("scope is live").clone();
        self.interp
            .as_ref()
            .expect("interpreter is live")
            .enter(|vm| f(vm, scope))
    }
}

impl Drop for ReplState {
    fn drop(&mut self) {
        // Drop the scope inside the VM context so Python __del__ methods can run,
        // then finalize the interpreter to release VM-owned resources.
        let scope = self.scope.take();
        if let Some(interp) = self.interp.take() {
            interp.enter(|_| drop(scope));
            let _ = interp.finalize(None);
        } else {
            drop(scope);
        }
    }
}

/// Create a new REPL session, optionally preloading `source`.
///
/// If `source` is non-empty it is executed into the scope so its definitions
/// are available to subsequent `repl_run` calls. Type errors are printed to
/// stderr and execution continues (the REPL is still created).
pub fn repl_new(source: &str, level: Level) -> Box<ReplState> {
    let has_source = !source.trim().is_empty();
    let interp = new_interpreter();
    let scope = interp.enter(|vm| {
        let scope = vm
            .new_scope_with_main()
            .expect("creating the main scope should not fail");
        register_ffi_module(vm);
        #[cfg(feature = "capture")]
        install_capture_writers(vm);
        #[cfg(target_arch = "wasm32")]
        if let Err(exc) = vm.run_string(
            scope.clone(),
            "from spython.system import install_displayhook; install_displayhook(); del install_displayhook",
            "<init>",
        ) {
            vm.print_exception(exc);
        }
        if has_source {
            if let Err(exc) = vm.run_string(scope.clone(), source, "user.py") {
                vm.print_exception(exc);
            } else {
                // Run doctests using the custom runner (avoids stdlib doctest
                // which needs _io.FileIO, unavailable on WASM).
                let doctest_code = concat!(
                    include_str!("doctest_runner.py"),
                    "\nimport __main__; run_doctests(__main__); del __main__\n"
                );
                if let Err(exc) = vm.run_string(scope.clone(), doctest_code, "<doctest>")
                {
                    vm.print_exception(exc);
                }
            }
        }
        scope
    });
    let accumulated_source = if has_source {
        source.to_owned()
    } else {
        String::new()
    };
    Box::new(ReplState {
        scope: Some(scope),
        interp: Some(interp),
        accumulated_source,
        level,
    })
}

/// Return values from `repl_run`.
pub const REPL_OK: u32 = 0;
pub const REPL_ERROR: u32 = 1;
pub const REPL_QUIT: u32 = 2;

fn format_elapsed(elapsed: std::time::Duration) -> String {
    if elapsed.as_secs() > 0 {
        format!("{:.2} s", elapsed.as_secs_f64())
    } else if elapsed.as_millis() > 0 {
        format!("{} ms", elapsed.as_millis())
    } else if elapsed.as_micros() > 0 {
        format!("{} us", elapsed.as_micros())
    } else {
        format!("{} ns", elapsed.as_nanos())
    }
}

/// Execute one REPL expression/statement in the session's scope.
///
/// Returns `REPL_OK` on success, `REPL_ERROR` on type/runtime error, or
/// `REPL_QUIT` if the user called `exit()` / `quit()` (SystemExit raised).
pub fn repl_run(state: &mut ReplState, code: &str) -> u32 {
    // Handle :type / :time commands (works in both CLI and WASM).
    let trimmed = code.trim();
    if let Some(expr) = trimmed.strip_prefix(":type ") {
        match infer_expression_type(&state.accumulated_source, expr) {
            Some(ty) => println!("{ty}"),
            None => eprintln!("Could not infer type"),
        }
        return REPL_OK;
    }
    if trimmed == ":type" {
        eprintln!("Usage: :type <expression>");
        return REPL_OK;
    }

    if trimmed == ":time" {
        eprintln!("Usage: :time <expression>");
        return REPL_OK;
    }

    let (code, timed) = if let Some(expr) = trimmed.strip_prefix(":time ") {
        if expr.trim().is_empty() {
            eprintln!("Usage: :time <expression>");
            return REPL_OK;
        }
        (format!("{expr}\n"), true)
    } else {
        (code.to_owned(), false)
    };

    if !type_check_repl_input(&state.accumulated_source, &code, state.level, true) {
        return REPL_ERROR;
    }

    let scope = state.scope.as_ref().unwrap().clone();
    let start = timed.then(std::time::Instant::now);
    let result = state
        .interp
        .as_ref()
        .expect("interpreter is live")
        .enter(|vm| {
            match vm
                .compile(&code, vm::compiler::Mode::Single, "<stdin>")
                .map_err(|err| err.into_pyexception(vm, Some(&code)))
                .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
            {
                Ok(_) => REPL_OK,
                Err(exc) => {
                    if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                        REPL_QUIT
                    } else {
                        vm.print_exception(exc);
                        REPL_ERROR
                    }
                }
            }
        });

    if result == REPL_OK {
        state.append_source(&code);
        if let Some(start) = start {
            println!("Time: {}", format_elapsed(start.elapsed()));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::REPL_ATEXIT_RAN;

    #[test]
    fn repl_drop_runs_atexit_handlers() {
        REPL_ATEXIT_RAN.store(false, Ordering::SeqCst);

        let source = format!(
            "import atexit\n\
from _spython_ffi import __test_mark_atexit_ran\n\
atexit.register(__test_mark_atexit_ran)\n"
        );

        {
            let _repl = repl_new(&source, Level::Full);
        }

        assert!(
            REPL_ATEXIT_RAN.load(Ordering::SeqCst),
            "expected atexit handler to run on repl drop"
        );
    }
}
