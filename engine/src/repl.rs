//! One REPL input at a time, in the state the previous ones left.
//!
//! A `Repl` owns the interpreter, the scope, and everything the checker needs
//! to read the next input in the light of the ones before it: the source that
//! has run, the level, and the files it imports.

use crate::Level;
use crate::completion::TabAction;
use crate::ffi::register_ffi_module;
use crate::run::{DOCTEST_RUNNER, keep_source, new_interpreter};
use crate::{ReplProject, infer_expression_type, type_check_repl_input};
use ruff_python_ast::PySourceType;
use ruff_python_parser::parse_unchecked_source;
use ruff_python_stdlib::identifiers::is_identifier;
use ruff_python_stdlib::keyword::is_keyword;
use ruff_text_size::{Ranged, TextLen, TextRange};
use rustpython::vm;
use rustpython::vm::AsObject;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// The input did not run, and why is already on the screen.
#[derive(Debug)]
pub struct Failed;

/// Why [`Repl::type_of`] has no type to give.
#[derive(Debug, PartialEq, Eq)]
pub enum NoType {
    /// The checker has already printed why.
    Shown,
    /// The text is not one expression.
    NotAnExpression,
    /// The checker reads no type off the expression.
    Unknown,
}

/// How a file that [`Repl::load`] ran came to its end.
#[derive(Debug, PartialEq, Eq)]
pub enum Loaded {
    /// It ran to the last line.
    Ran,
    /// It called `exit()` with this status.
    Exited(u32),
}

/// The project of a session, which is made on the first check of a session
/// that did not start from a file.
fn project(project: &mut Option<ReplProject>) -> &mut ReplProject {
    project.get_or_insert_with(|| ReplProject::new(&[]))
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
    /// The project the inputs are checked in, made on the first check. It
    /// holds the first-party files the accumulated source imports. The
    /// checker runs on an in-memory filesystem, so an input that names one of
    /// them only resolves if it is there. The page has a single file, so its
    /// project holds none.
    project: Option<ReplProject>,
    /// The status of the `exit()` that ended the session, if one has run.
    exit_status: Option<u32>,
    /// How many inputs have reached the interpreter. Each one runs under a
    /// file name of its own, as in the Python REPL, so that a traceback finds
    /// its lines.
    inputs: usize,
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
            project: None,
            exit_status: None,
            inputs: 0,
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
    /// there, so a file that runs to its end is the context the next inputs
    /// are checked in.
    ///
    /// A file that raises or calls `exit()` stops at a line the session
    /// cannot know without running it in pieces, which would change what its
    /// module holds. So the checker gets nothing of it, and the names it bound
    /// are unknown to the next inputs. The `exit()` comes back as
    /// [`Loaded::Exited`] and does not end the session. The page has no way
    /// to end a session that `repl_new` is still opening, so each caller
    /// decides.
    pub fn load(
        &mut self,
        source: &str,
        name: &str,
        dir: Option<&str>,
        modules: Vec<(String, String)>,
    ) -> Result<Loaded, Failed> {
        self.project = Some(ReplProject::new(&modules));
        let (_, scope) = self.live();
        let loaded = self.enter(|vm| {
            if let Some(dir) = dir
                && let Err(exc) = vm.insert_sys_path(vm.new_pyobj(dir))
            {
                vm.print_exception(exc);
                return Err(Failed);
            }
            keep_source(vm, name, source);
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
        });
        if let Ok(Loaded::Ran) = loaded {
            self.accumulated_source = source.to_owned();
        }
        loaded
    }

    /// Run the doctests of the source that has run in the session, with the
    /// runner that ships with spython. The stdlib one needs `_io.FileIO`,
    /// which the page has not got.
    ///
    /// The runner has a scope of its own, so that its names stay out of the
    /// session and leave the definitions of the student as they are.
    pub fn run_doctests(&mut self) -> Result<(), Failed> {
        let code = format!("{DOCTEST_RUNNER}\nimport __main__\nrun_doctests(__main__, _source)\n");
        self.enter(|vm| {
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

    /// The status of the `exit()` that ended the session, if one has run.
    pub fn exit_status(&self) -> Option<u32> {
        self.exit_status
    }

    /// The type `expr` would have, without running it. `expr` has to be one
    /// expression on its own, or the history would answer for it.
    pub fn type_of(&mut self, expr: &str) -> Result<String, NoType> {
        let parsed = parse_unchecked_source(expr, PySourceType::Python);
        if !parsed.has_valid_syntax() {
            // The checker says where the syntax breaks. The first line of
            // `expr` starts at column 0 and closes whatever the history left
            // open, so the error is in `expr` there too.
            let shown = !type_check_repl_input(
                project(&mut self.project),
                &self.accumulated_source,
                expr,
                self.level,
                self.use_color,
            );
            return Err(if shown {
                NoType::Shown
            } else {
                NoType::NotAnExpression
            });
        }
        if !matches!(parsed.suite().as_slice(), [ruff_python_ast::Stmt::Expr(_)]) {
            return Err(NoType::NotAnExpression);
        }
        infer_expression_type(project(&mut self.project), &self.accumulated_source, expr)
            .ok_or(NoType::Unknown)
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

    /// Run code of the student, then end the world it left open and write out
    /// its output, in that order, so the output comes after the last frame.
    fn enter<R>(&self, run: impl FnOnce(&vm::VirtualMachine) -> R) -> R {
        let (interp, _) = self.live();
        let result = interp.enter(run);
        crate::display::close_frames();
        flush_output(interp);
        result
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
    /// It is checked against everything that ran before it. Its top-level
    /// statements run one at a time, as the Python REPL would run them typed
    /// one per prompt, and each joins the accumulated source once it has run.
    /// A statement that raises stops the input there, so the source holds
    /// what ran and nothing after it.
    pub fn run(&mut self, code: &str) -> Result<Duration, Failed> {
        // `Mode::Single` wants the line the input ends on to be closed.
        let code = if code.ends_with('\n') {
            code.to_owned()
        } else {
            format!("{code}\n")
        };

        if !type_check_repl_input(
            project(&mut self.project),
            &self.accumulated_source,
            &code,
            self.level,
            self.use_color,
        ) {
            return Err(Failed);
        }

        let parsed = parse_unchecked_source(&code, PySourceType::Python);
        // The checker has refused an input that does not parse, so the whole
        // input as one piece is only there to let the compiler say why.
        let statements: Vec<TextRange> = if parsed.has_valid_syntax() {
            parsed.suite().iter().map(Ranged::range).collect()
        } else {
            vec![TextRange::up_to(code.text_len())]
        };

        let name = format!("<python-input-{}>", self.inputs);
        self.inputs += 1;
        let (_, scope) = self.live();
        let (ran, elapsed) = self.enter(|vm| {
            let start = Instant::now();
            let ran = 'ran: {
                keep_source(vm, &name, &code);
                // Every statement compiles before the first one runs, so a
                // compile error leaves nothing half run, and the warnings come
                // out ahead of the output.
                let mut compiled = Vec::with_capacity(statements.len());
                for &range in &statements {
                    let source = alone(&code, range);
                    match vm
                        .compile(&source, vm::compiler::Mode::Single, name.as_str())
                        .map_err(|err| err.into_pyexception(vm, Some(&source)))
                    {
                        Ok(code_obj) => compiled.push(code_obj),
                        Err(exc) => {
                            vm.print_exception(exc);
                            break 'ran Ran {
                                done: 0,
                                exit: None,
                                changed: Vec::new(),
                            };
                        }
                    }
                }
                for (done, code_obj) in compiled.into_iter().enumerate() {
                    let before = Snapshot::take(&scope.globals, vm);
                    if let Err(exc) = vm.run_code_obj(code_obj, scope.clone()) {
                        let changed = before.changed(&scope.globals);
                        let exit = if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                            // Says what the status is, and writes the message of
                            // an `exit("...")` that carries one.
                            Some(vm.handle_exit_exception(exc))
                        } else {
                            vm.print_exception(exc);
                            None
                        };
                        break 'ran Ran {
                            done,
                            exit,
                            changed,
                        };
                    }
                }
                Ran {
                    done: statements.len(),
                    exit: None,
                    changed: Vec::new(),
                }
            };
            (ran, start.elapsed())
        });

        if ran.done == statements.len() && ran.exit.is_none() {
            self.append_source(&code);
            return Ok(elapsed);
        }
        if let Some(last) = ran.done.checked_sub(1).and_then(|at| statements.get(at)) {
            self.append_source(&code[..last.end().to_usize()]);
        }
        if !ran.changed.is_empty() {
            let dels: Vec<String> = ran
                .changed
                .iter()
                .map(|name| format!("del {name}"))
                .collect();
            self.append_source(&dels.join("\n"));
        }
        if let Some(status) = ran.exit {
            self.exit_status = Some(status);
        }
        Err(Failed)
    }
}

/// Write out what `sys.stdout` and `sys.stderr` hold. Away from a terminal
/// both keep a buffer, and what the REPL prints itself would come out ahead
/// of the output of the inputs before it.
fn flush_output(interp: &vm::Interpreter) {
    interp.enter(|vm| {
        for name in ["stdout", "stderr"] {
            if let Ok(stream) = vm.sys_module.get_attr(name, vm) {
                // A stream the program closed or broke has nothing to write.
                let _ = vm.call_method(&stream, "flush", ());
            }
        }
    });
}

/// How far an input went: the number of its top-level statements that ran to
/// the end, the status of the `exit()` that stopped it, if one did, and the
/// global names the statement that raised has bound, rebound or deleted.
struct Ran {
    done: usize,
    exit: Option<u32>,
    changed: Vec<String>,
}

/// The global names and what each one held, to tell afterwards which names a
/// statement changed.
///
/// A value that takes a weak reference is held by one, so that the snapshot
/// does not keep it alive and its `__del__` runs when Python would run it.
/// The other values are held as they are. Comparing addresses alone would
/// not do, as a new value can take the address of one that has been freed.
struct Snapshot(Vec<(String, Held)>);

enum Held {
    Weak(vm::PyRef<vm::builtins::PyWeak>),
    Strong(vm::PyObjectRef),
}

impl Snapshot {
    fn take(globals: &vm::builtins::PyDictRef, vm: &vm::VirtualMachine) -> Snapshot {
        let names = globals
            .into_iter()
            .filter_map(|(key, value)| {
                let name = key
                    .downcast_ref::<vm::builtins::PyStr>()?
                    .to_str()?
                    .to_owned();
                let held = match value.downgrade(None, vm) {
                    Ok(weak) => Held::Weak(weak),
                    Err(_) => Held::Strong(value),
                };
                Some((name, held))
            })
            .collect();
        Snapshot(names)
    }

    /// The names whose value in `globals` is not the one the snapshot holds,
    /// the names `globals` has gained or lost included, sorted. A name no code
    /// can spell, such as a key that `globals()` received, stays out.
    fn changed(&self, globals: &vm::builtins::PyDictRef) -> Vec<String> {
        let mut now: HashMap<String, vm::PyObjectRef> = globals
            .into_iter()
            .filter_map(|(key, value)| {
                let name = key
                    .downcast_ref::<vm::builtins::PyStr>()?
                    .to_str()?
                    .to_owned();
                Some((name, value))
            })
            .collect();
        let mut changed: Vec<String> = Vec::new();
        for (name, held) in &self.0 {
            let same = now.remove(name).is_some_and(|value| match held {
                Held::Weak(weak) => weak.upgrade().is_some_and(|old| old.is(&value)),
                Held::Strong(old) => old.is(&value),
            });
            if !same {
                changed.push(name.clone());
            }
        }
        changed.extend(now.into_keys());
        changed.retain(|name| is_identifier(name) && !is_keyword(name));
        changed.sort();
        changed
    }
}

/// The statement of `code` at `range`, on the line it has in `code`, so that
/// a traceback names the line the user typed.
fn alone(code: &str, range: TextRange) -> String {
    let line = code[..range.start().to_usize()].matches('\n').count();
    format!("{}{}\n", "\n".repeat(line), &code[range])
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
    use ruff_text_size::TextSize;

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
        assert!(repl.run("y = 1\n").is_ok());
    }

    #[test]
    fn a_file_that_stops_partway_leaves_its_names_unknown() {
        let mut repl = Repl::new(Level::Full, false);
        let loaded = repl.load("x = 1\nint(\"z\")\ny = 2\n", "user.py", None, Vec::new());
        assert!(loaded.is_err());
        assert!(repl.run("y\n").is_err());
        assert!(repl.run("x\n").is_err());
        let _ = repl.load("x = 1\nexit()\ny = 2\n", "user.py", None, Vec::new());
        assert!(repl.run("y\n").is_err());
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
    fn an_input_that_raises_keeps_the_statements_that_ran() {
        let mut repl = Repl::new(Level::Full, false);
        assert!(repl.run("x = 1\n").is_ok());
        assert!(repl.run("x = \"a\"\nint(\"z\")\ny = 1\n").is_err());
        assert!(repl.run("assert x.upper() == \"A\"\n").is_ok());
        assert!(repl.run("x + 1\n").is_err());
        assert!(repl.run("y\n").is_err());
    }

    #[test]
    fn a_name_a_statement_that_raised_has_rebound_is_forgotten() {
        let mut repl = Repl::new(Level::Full, false);
        assert!(repl.run("x = 1\n").is_ok());
        assert!(repl.run("for x in [\"a\"]:\n    int(\"z\")\n").is_err());
        assert!(repl.run("x\n").is_err());
        assert!(repl.run("x = 2\n").is_ok());
        assert!(repl.run("assert x + 1 == 3\n").is_ok());
    }

    #[test]
    fn type_of_answers_only_for_one_expression() {
        let mut repl = Repl::new(Level::Full, false);
        assert!(repl.run("\"abc\"\n").is_ok());
        assert_eq!(repl.type_of("1"), Ok("int".to_owned()));
        assert_eq!(repl.type_of("# c"), Err(NoType::NotAnExpression));
        assert_eq!(repl.type_of("1; \"a\""), Err(NoType::NotAnExpression));
        assert_eq!(repl.type_of("x = 1"), Err(NoType::NotAnExpression));
        assert_eq!(repl.type_of("1 +"), Err(NoType::Shown));
    }

    #[test]
    fn completion_leaves_out_a_name_with_a_surrogate() {
        let mut repl = Repl::new(Level::Full, false);
        let code = "class A:\n    x: int = 1\n\na = A()\nsetattr(a, \"\\ud800\", 1)\n";
        assert!(repl.run(code).is_ok());
        let action = repl.complete("a.", 2, &[]);
        assert!(matches!(action, TabAction::Complete(2, ref c) if c == &["x"]));
    }

    #[test]
    fn a_name_a_statement_that_raised_has_left_alone_is_kept() {
        let mut repl = Repl::new(Level::Full, false);
        assert!(repl.run("x = 1\n").is_ok());
        assert!(repl.run("x = int(\"z\")\n").is_err());
        assert!(repl.run("assert x + 1 == 2\n").is_ok());
    }

    #[test]
    fn the_value_a_statement_replaces_goes_when_python_lets_it_go() {
        let mut repl = Repl::new(Level::Full, false);
        let class = "log: list[str] = []\n\
class A:\n    def __del__(self) -> None:\n        log.append(\"del\")\n\n\
a = A()\n";
        assert!(repl.run(class).is_ok());
        assert!(
            repl.run("for _ in [1]:\n    a = A()\n    log.append(\"x\")\n")
                .is_ok()
        );
        assert!(repl.run("assert log == [\"del\", \"x\"], log\n").is_ok());
    }

    #[test]
    fn a_statement_runs_on_the_line_it_was_typed_on() {
        let code = "x = 1\nif x:\n    y = 2\n";
        let range = TextRange::new(TextSize::new(6), code.text_len() - TextSize::new(1));
        assert_eq!(alone(code, range), "\nif x:\n    y = 2\n");
    }

    #[test]
    fn load_of_a_file_that_ends_returns_ran() {
        let mut repl = Repl::new(Level::Full, false);
        let loaded = repl.load("x = 1\n", "user.py", None, Vec::new());
        assert_eq!(loaded.ok(), Some(Loaded::Ran));
    }
}
