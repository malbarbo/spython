//! One REPL input at a time, in the state the previous ones left.
//!
//! A `Repl` owns the interpreter, the scope, and everything the checker needs
//! to read the next input in the light of the ones before it: the source that
//! has run, the level, and the files it imports.

use crate::Level;
use crate::completion::TabAction;
use crate::ffi::register_ffi_module;
use crate::run::{DOCTEST_RUNNER, new_interpreter};
use crate::{infer_expression_type, type_check_repl_input};
use rustpython::vm;
use rustpython::vm::AsObject;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// The input did not run, and why is already on the screen.
#[derive(Debug)]
pub struct Failed;

/// How a file that [`Repl::load`] ran came to its end.
#[derive(Debug, PartialEq, Eq)]
pub enum Loaded {
    /// It ran to the last line.
    Ran,
    /// It called `exit()` with this status.
    Exited(u32),
}

/// A REPL session and the interpreter it runs in.
pub struct Repl {
    // `scope` is wrapped in `Option` so the `Drop` impl can take it and
    // drop it inside the VM context (see below).
    scope: Option<vm::scope::Scope>,
    interp: Option<vm::Interpreter>,
    /// Accumulated source of all successfully type-checked and executed inputs.
    accumulated_source: String,
    /// Teaching level for annotation/construct checking.
    level: Level,
    /// Whether diagnostics carry ANSI colour. True on the page, which renders
    /// the codes; a terminal only wants them when stderr is one.
    use_color: bool,
    /// The first-party files the accumulated source imports, each a path
    /// relative to the directory it came from and its text. The checker runs
    /// on an in-memory filesystem, so an input that names one of them only
    /// resolves if it is here. Empty on the page, which has a single file.
    modules: Vec<(String, String)>,
    /// The status of the `exit()` that ended the session, if one has run.
    exit_status: Option<u32>,
}

impl Repl {
    /// An interpreter with an empty scope, checking what it takes at `level`.
    pub fn new(level: Level, use_color: bool) -> Repl {
        let interp = new_interpreter();
        let scope = interp.enter(|vm| {
            let scope = vm
                .new_scope_with_main()
                .expect("creating the main scope should not fail");
            register_ffi_module(vm);
            scope
        });
        Repl {
            scope: Some(scope),
            interp: Some(interp),
            accumulated_source: String::new(),
            level,
            use_color,
            modules: Vec::new(),
            exit_status: None,
        }
    }

    /// Show an image as a picture, in svg on the page and in a terminal that
    /// shows graphics. Any other value, and an image in a terminal that
    /// shows none, prints as Python prints it.
    pub fn install_displayhook(&self) {
        let (interp, scope) = self.live();
        interp.enter(|vm| {
            if let Err(exc) = vm.run_string(
                scope,
                "from spython.system import install_displayhook; install_displayhook(); del install_displayhook",
                "<init>",
            ) {
                vm.print_exception(exc);
            }
        });
    }

    /// Run `source` under the name `name`, with `dir` on `sys.path` and
    /// `modules` the first-party files it imports. The session goes on from
    /// there, so what runs here is the context the next inputs are checked in.
    ///
    /// An `exit()` in `source` comes back as [`Loaded::Exited`] and does not
    /// end the session. The page has no way to end a session that `repl_new`
    /// is still opening, so each caller decides.
    pub fn load(
        &mut self,
        source: &str,
        name: &str,
        dir: Option<&str>,
        modules: Vec<(String, String)>,
    ) -> Result<Loaded, Failed> {
        self.modules = modules;
        self.accumulated_source = source.to_owned();
        let (interp, scope) = self.live();
        interp.enter(|vm| {
            if let Some(dir) = dir
                && let Err(exc) = vm.insert_sys_path(vm.new_pyobj(dir))
            {
                vm.print_exception(exc);
                return Err(Failed);
            }
            match vm.run_string(scope, source, name) {
                Ok(_) => Ok(Loaded::Ran),
                // Writes the message of an `exit("...")` that carries one.
                Err(exc) if exc.fast_isinstance(vm.ctx.exceptions.system_exit) => {
                    Ok(Loaded::Exited(vm.handle_exit_exception(exc)))
                }
                Err(exc) => {
                    vm.print_exception(exc);
                    Err(Failed)
                }
            }
        })
    }

    /// Run the doctests of the source that has run in the session, with the
    /// runner that ships with spython. The stdlib one needs `_io.FileIO`,
    /// which the page has not got.
    ///
    /// The runner has a scope of its own, so that its names stay out of the
    /// session and leave the definitions of the student as they are.
    pub fn run_doctests(&mut self) -> Result<(), Failed> {
        let code = format!("{DOCTEST_RUNNER}\nimport __main__\nrun_doctests(__main__, _source)\n");
        let (interp, _) = self.live();
        interp.enter(|vm| {
            let scope = vm.new_scope_with_builtins();
            let run = scope
                .globals
                .set_item(
                    "_source",
                    vm.ctx.new_str(self.accumulated_source.as_str()).into(),
                    vm,
                )
                .and_then(|()| vm.run_string(scope, &code, "<doctest>"));
            match run {
                Ok(_) => Ok(()),
                Err(exc) => {
                    vm.print_exception(exc);
                    Err(Failed)
                }
            }
        })
    }

    /// The teaching level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Check the inputs that follow at `level`, if everything that has
    /// already run survives it. Says whether it did; on `false` the level has
    /// not moved and the diagnostics have been printed.
    pub fn set_level(&mut self, level: Level) -> bool {
        if !self.accumulated_source.is_empty()
            && !type_check_repl_input(
                "",
                &self.accumulated_source,
                level,
                self.use_color,
                &self.modules,
            )
        {
            return false;
        }
        self.level = level;
        true
    }

    /// The status of the `exit()` that ended the session, if one has run.
    pub fn exit_status(&self) -> Option<u32> {
        self.exit_status
    }

    /// The type `expr` would have, without running it, and `None` for an
    /// expression the checker cannot read a type off.
    pub fn type_of(&self, expr: &str) -> Option<String> {
        infer_expression_type(&self.accumulated_source, expr, &self.modules)
    }

    /// What the Tab key should do at `cursor`, over this session's names and
    /// the `commands` the shell answers.
    pub fn complete(&self, text: &str, cursor: usize, commands: &[(&str, bool)]) -> TabAction {
        let (interp, scope) = self.live();
        interp.enter(|vm| crate::completion::tab_action(vm, &scope.globals, text, cursor, commands))
    }

    /// The interpreter and the scope, which only a `Repl` on its way out has
    /// let go of.
    fn live(&self) -> (&vm::Interpreter, vm::scope::Scope) {
        (
            self.interp.as_ref().expect("interpreter is live"),
            self.scope.as_ref().expect("scope is live").clone(),
        )
    }

    /// Append successfully executed source to the accumulator.
    fn append_source(&mut self, source: &str) {
        if !self.accumulated_source.is_empty() {
            self.accumulated_source.push('\n');
        }
        self.accumulated_source.push_str(source);
    }

    /// Type-check, run and remember one piece of Python, and say how long the
    /// interpreter took over it.
    ///
    /// It is checked against everything that ran before it, and only joins
    /// the accumulated source once it has run.
    pub fn run(&mut self, code: &str) -> Result<Duration, Failed> {
        // `Mode::Single` wants the line the input ends on to be closed.
        let code = if code.ends_with('\n') {
            code.to_owned()
        } else {
            format!("{code}\n")
        };

        if !type_check_repl_input(
            &self.accumulated_source,
            &code,
            self.level,
            self.use_color,
            &self.modules,
        ) {
            return Err(Failed);
        }

        let (interp, scope) = self.live();
        let start = Instant::now();
        // `Err` is the status of an `exit()`, which only the session itself
        // can record, and the borrow of the interpreter has to end first.
        let ran: Result<bool, u32> = interp.enter(|vm| {
            match vm
                .compile(&code, vm::compiler::Mode::Single, "<stdin>")
                .map_err(|err| err.into_pyexception(vm, Some(&code)))
                .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
            {
                Ok(_) => Ok(true),
                Err(exc) => {
                    if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                        // Says what the status is, and writes the message of
                        // an `exit("...")` that carries one.
                        Err(vm.handle_exit_exception(exc))
                    } else {
                        vm.print_exception(exc);
                        Ok(false)
                    }
                }
            }
        });
        let elapsed = start.elapsed();

        match ran {
            Err(status) => {
                self.exit_status = Some(status);
                Err(Failed)
            }
            Ok(false) => Err(Failed),
            Ok(true) => {
                self.append_source(&code);
                Ok(elapsed)
            }
        }
    }
}

impl Drop for Repl {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::REPL_ATEXIT_RAN;

    #[test]
    fn repl_drop_runs_atexit_handlers() {
        REPL_ATEXIT_RAN.store(false, Ordering::SeqCst);

        let source = "import atexit\n\
from _spython_ffi import __test_mark_atexit_ran\n\
atexit.register(__test_mark_atexit_ran)\n";

        {
            let mut repl = Repl::new(Level::Full, false);
            let _ = repl.load(source, "user.py", None, Vec::new());
        }

        assert!(
            REPL_ATEXIT_RAN.load(Ordering::SeqCst),
            "expected atexit handler to run on repl drop"
        );
    }

    #[test]
    fn load_returns_the_status_of_an_exit() {
        let mut repl = Repl::new(Level::Full, false);
        let loaded = repl.load("x = 1\nexit(3)\n", "user.py", None, Vec::new());
        assert_eq!(loaded.ok(), Some(Loaded::Exited(3)));
    }

    #[test]
    fn load_after_an_exit_leaves_the_session_open() {
        let mut repl = Repl::new(Level::Full, false);
        let _ = repl.load("x = 1\nexit()\n", "user.py", None, Vec::new());
        assert_eq!(repl.exit_status(), None);
        assert!(repl.run("y = x\n").is_ok());
    }

    #[test]
    fn run_doctests_leaves_the_names_of_the_session_alone() {
        let mut repl = Repl::new(Level::Full, false);
        let source =
            "def _parse() -> int:\n    \"\"\"\n    >>> _parse()\n    1\n    \"\"\"\n    return 1\n";
        let _ = repl.load(source, "user.py", None, Vec::new());
        assert!(repl.run_doctests().is_ok());
        assert!(repl.run("assert _parse() == 1\n").is_ok());
        let (interp, scope) = repl.live();
        interp.enter(|vm| {
            assert!(matches!(
                scope.globals.get_item_opt("run_doctests", vm),
                Ok(None)
            ));
        });
    }

    #[test]
    fn load_of_a_file_that_ends_returns_ran() {
        let mut repl = Repl::new(Level::Full, false);
        let loaded = repl.load("x = 1\n", "user.py", None, Vec::new());
        assert_eq!(loaded.ok(), Some(Loaded::Ran));
    }
}
