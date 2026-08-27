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

/// The REPL state that does not need the interpreter: what has run so far,
/// the level it is checked at, and whether its errors are coloured.
///
/// A frontend that already owns a VM -- the CLI, whose line editor borrows it
/// -- keeps one of these and calls [`ReplSession::run`] for every input.
pub struct ReplSession {
    /// Accumulated source of all successfully type-checked and executed inputs.
    accumulated_source: String,
    /// Teaching level for annotation/construct checking.
    level: Level,
    /// Whether diagnostics carry ANSI colour. True on the page, which renders
    /// the codes; a terminal only wants them when stderr is one.
    use_color: bool,
}

/// What running one input did.
pub enum ReplOutcome {
    /// The input ran.
    Ok,
    /// The input did not type-check, or it raised.
    Error,
    /// The input called `exit()` or `quit()`. The exception says with which
    /// status the process should leave.
    Quit(vm::builtins::PyBaseExceptionRef),
}

impl ReplSession {
    /// A session that starts as if `accumulated_source` had already run.
    pub fn new(accumulated_source: String, level: Level, use_color: bool) -> Self {
        ReplSession {
            accumulated_source,
            level,
            use_color,
        }
    }

    /// The accumulated source of all successfully type-checked and executed inputs.
    pub fn accumulated_source(&self) -> &str {
        &self.accumulated_source
    }

    /// The teaching level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Check the inputs that follow at `level`.
    pub fn set_level(&mut self, level: Level) {
        self.level = level;
    }

    /// Append successfully executed source to the accumulator.
    fn append_source(&mut self, source: &str) {
        if !self.accumulated_source.is_empty() {
            self.accumulated_source.push('\n');
        }
        self.accumulated_source.push_str(source);
    }

    /// Execute one REPL expression, statement or command in `scope`.
    ///
    /// The input is type-checked against everything that ran before it, and
    /// only joins the accumulated source once it has run.
    pub fn run(
        &mut self,
        vm: &vm::VirtualMachine,
        scope: vm::scope::Scope,
        code: &str,
    ) -> ReplOutcome {
        let trimmed = code.trim();
        if let Some(expr) = trimmed.strip_prefix(":type ") {
            match infer_expression_type(&self.accumulated_source, expr) {
                Some(ty) => println!("{ty}"),
                None => eprintln!("Could not infer type"),
            }
            return ReplOutcome::Ok;
        }
        if trimmed == ":type" {
            eprintln!("Usage: :type <expression>");
            return ReplOutcome::Ok;
        }
        if trimmed == ":time" {
            eprintln!("Usage: :time <expression>");
            return ReplOutcome::Ok;
        }

        let (code, timed) = if let Some(expr) = trimmed.strip_prefix(":time ") {
            if expr.trim().is_empty() {
                eprintln!("Usage: :time <expression>");
                return ReplOutcome::Ok;
            }
            (format!("{expr}\n"), true)
        } else if code.ends_with('\n') {
            (code.to_owned(), false)
        } else {
            // `Mode::Single` wants the line the input ends on to be closed.
            (format!("{code}\n"), false)
        };

        if !type_check_repl_input(&self.accumulated_source, &code, self.level, self.use_color) {
            return ReplOutcome::Error;
        }

        let start = timed.then(std::time::Instant::now);
        let outcome = match vm
            .compile(&code, vm::compiler::Mode::Single, "<stdin>")
            .map_err(|err| err.into_pyexception(vm, Some(&code)))
            .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
        {
            Ok(_) => ReplOutcome::Ok,
            Err(exc) => {
                if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                    ReplOutcome::Quit(exc)
                } else {
                    vm.print_exception(exc);
                    ReplOutcome::Error
                }
            }
        };

        if matches!(outcome, ReplOutcome::Ok) {
            self.append_source(&code);
            if let Some(start) = start {
                println!("Time: {}", format_elapsed(start.elapsed()));
            }
        }

        outcome
    }
}

/// A REPL session and the interpreter it runs in.
///
/// The page has no VM of its own, so this owns one and hands the session an
/// input at a time through [`repl_run`].
pub struct ReplState {
    // `scope` is wrapped in `Option` so the `Drop` impl can take it and
    // drop it inside the VM context (see below).
    scope: Option<vm::scope::Scope>,
    interp: Option<vm::Interpreter>,
    session: ReplSession,
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
        // The page renders the ANSI codes, so its diagnostics keep them.
        session: ReplSession::new(accumulated_source, level, true),
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

/// Execute one REPL input in the page's session.
///
/// Returns `REPL_OK` on success, `REPL_ERROR` on type/runtime error, or
/// `REPL_QUIT` if the user called `exit()` / `quit()` (SystemExit raised).
pub fn repl_run(state: &mut ReplState, code: &str) -> u32 {
    // Borrowing the fields apart lets the session be mutable while the
    // interpreter it runs in is only read.
    let interp = state.interp.as_ref().expect("interpreter is live");
    let scope = state.scope.as_ref().expect("scope is live").clone();
    let session = &mut state.session;
    match interp.enter(|vm| session.run(vm, scope, code)) {
        ReplOutcome::Ok => REPL_OK,
        ReplOutcome::Error => REPL_ERROR,
        ReplOutcome::Quit(_) => REPL_QUIT,
    }
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
