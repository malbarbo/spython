//! One REPL input at a time, in the state the previous ones left.
//!
//! A `Repl` owns the interpreter, the scope, and everything the checker needs
//! to read the next input in the light of the ones before it: the source that
//! has run, the level, and the files it imports.

use crate::Level;
use crate::completion::TabAction;
use crate::ffi::register_ffi_module;
use crate::shell::{Arg, BUILTINS, Builtin, split};
use crate::{infer_expression_type, new_interpreter, type_check_repl_input};
use rustpython::vm;
use rustpython::vm::AsObject;
use std::convert::Infallible;
#[cfg(test)]
use std::sync::atomic::Ordering;

/// A `:` command the REPL answers.
struct Command<H> {
    name: &'static str,
    arg: Arg,
    help: &'static str,
    action: Action<H>,
}

/// Who answers a command: the REPL itself, or the frontend that registered
/// it, which names it with a value of its own `H`.
#[derive(Clone, Copy)]
enum Action<H> {
    Builtin(Builtin),
    Host(H),
}

impl<H> Command<H> {
    fn usage(&self) -> String {
        usage(self.name, self.arg)
    }
}

/// How the help and the usage errors write a command.
fn usage(name: &str, arg: Arg) -> String {
    match arg {
        Arg::None => name.to_owned(),
        Arg::Expr => format!("{name} <expression>"),
        Arg::Word(word) => format!("{name} {word}"),
    }
}

/// The input did not run, and why is already on the screen.
#[derive(Debug)]
pub struct Failed;

/// A REPL session and the interpreter it runs in.
pub struct Repl<H = Infallible> {
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
    /// The `:` commands, the builtins plus whatever the frontend added.
    commands: Vec<Command<H>>,
    /// The status `exit()` or `quit()` asked the process to leave with.
    exit_status: Option<u32>,
}

/// What running one input did.
pub enum ReplOutcome<H = Infallible> {
    /// The input ran.
    Ok,
    /// The input did not type-check, or it raised.
    Error,
    /// The session is over: `:quit`, or `exit()` / `quit()`, which leave the
    /// status in [`Repl::exit_status`].
    Quit,
    /// A command the frontend registered, and what followed its name. Only
    /// the frontend knows what to do with it. The frontend names its commands
    /// with an enum, so leaving one unanswered is a compile error.
    Host(H, String),
}

impl<H: Copy> Repl<H> {
    /// An interpreter with an empty scope, checking what it takes at `level`.
    pub fn new(level: Level, use_color: bool) -> Repl<H> {
        let interp = new_interpreter();
        let scope = interp.enter(|vm| {
            let scope = vm
                .new_scope_with_main()
                .expect("creating the main scope should not fail");
            register_ffi_module(vm);
            #[cfg(feature = "capture")]
            crate::ffi::install_capture_writers(vm);
            #[cfg(target_arch = "wasm32")]
            if let Err(exc) = vm.run_string(
                scope.clone(),
                "from spython.system import install_displayhook; install_displayhook(); del install_displayhook",
                "<init>",
            ) {
                vm.print_exception(exc);
            }
            scope
        });
        Repl {
            scope: Some(scope),
            interp: Some(interp),
            accumulated_source: String::new(),
            level,
            use_color,
            modules: Vec::new(),
            commands: BUILTINS
                .iter()
                .map(|&(name, arg, help, builtin)| Command {
                    name,
                    arg,
                    help,
                    action: Action::Builtin(builtin),
                })
                .collect(),
            exit_status: None,
        }
    }

    /// Run `source` under the name `name`, with `dir` on `sys.path` and
    /// `modules` the first-party files it imports. The session goes on from
    /// there, so what runs here is the context the next inputs are checked in.
    pub fn load(
        &mut self,
        source: &str,
        name: &str,
        dir: Option<&str>,
        modules: Vec<(String, String)>,
    ) -> Result<(), Failed> {
        self.modules = modules;
        self.accumulated_source = source.to_owned();
        let scope = self.scope.as_ref().expect("scope is live").clone();
        let interp = self.interp.as_ref().expect("interpreter is live");
        interp.enter(|vm| {
            if let Some(dir) = dir
                && let Err(exc) = vm.insert_sys_path(vm.new_pyobj(dir))
            {
                vm.print_exception(exc);
                return Err(Failed);
            }
            match vm.run_string(scope, source, name) {
                Ok(_) => Ok(()),
                Err(exc) => {
                    vm.print_exception(exc);
                    Err(Failed)
                }
            }
        })
    }

    /// Run the doctests of what [`Repl::load`] loaded, with the runner that
    /// ships with spython. The stdlib one needs `_io.FileIO`, which the page
    /// has not got.
    pub fn run_doctests(&mut self) -> Result<(), Failed> {
        let code = concat!(
            include_str!("doctest_runner.py"),
            "\nimport __main__; run_doctests(__main__); del __main__\n"
        );
        let scope = self.scope.as_ref().expect("scope is live").clone();
        let interp = self.interp.as_ref().expect("interpreter is live");
        interp.enter(|vm| match vm.run_string(scope, code, "<doctest>") {
            Ok(_) => Ok(()),
            Err(exc) => {
                vm.print_exception(exc);
                Err(Failed)
            }
        })
    }

    /// Declare a command of the frontend's own, under the name `host` it
    /// knows it by. `word` is what the help shows after the name, for a
    /// command that takes a word. The session checks it and completes it like
    /// any other, and hands it back as [`ReplOutcome::Host`] to be answered.
    pub fn add(
        &mut self,
        name: &'static str,
        word: Option<&'static str>,
        help: &'static str,
        host: H,
    ) {
        debug_assert!(self.commands.iter().all(|c| c.name != name));
        self.commands.push(Command {
            name,
            arg: word.map_or(Arg::None, Arg::Word),
            help,
            action: Action::Host(host),
        });
    }

    /// Every command's name, sorted, and whether it takes anything after it.
    /// What a completer offers when the line so far is a colon, and what the
    /// terminal's highlighter matches a line against.
    pub fn command_names(&self) -> Vec<(&'static str, bool)> {
        let mut names: Vec<(&'static str, bool)> = self
            .commands
            .iter()
            .map(|c| (c.name, !matches!(c.arg, Arg::None)))
            .collect();
        names.sort();
        names
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

    /// The status `exit()` or `quit()` asked for, once one of them has run.
    pub fn exit_status(&self) -> Option<u32> {
        self.exit_status
    }

    /// What the Tab key should do at `cursor`, over this session's names and
    /// its commands.
    pub fn complete(&self, text: &str, cursor: usize) -> TabAction {
        let commands = self.command_names();
        let globals = &self.scope.as_ref().expect("scope is live").globals;
        self.interp
            .as_ref()
            .expect("interpreter is live")
            .enter(|vm| crate::completion::tab_action(vm, globals, text, cursor, &commands))
    }

    /// Append successfully executed source to the accumulator.
    fn append_source(&mut self, source: &str) {
        if !self.accumulated_source.is_empty() {
            self.accumulated_source.push('\n');
        }
        self.accumulated_source.push_str(source);
    }

    /// Execute one REPL input: a command, or Python.
    ///
    /// Python is type-checked against everything that ran before it, and only
    /// joins the accumulated source once it has run.
    pub fn run(&mut self, code: &str) -> ReplOutcome<H> {
        let Some((name, arg)) = split(code) else {
            return self.eval(code, false);
        };
        // Copied out of the table so the command can be answered with the
        // session, which the table lives in, borrowed again.
        let Some((command_arg, action)) = self
            .commands
            .iter()
            .find(|c| c.name == name)
            .map(|c| (c.arg, c.action))
        else {
            eprintln!("Unknown command {name}. Type :help to see the commands.");
            return ReplOutcome::Error;
        };
        match (command_arg, arg.is_empty()) {
            (Arg::None, false) | (Arg::Expr, true) => {
                eprintln!("Usage: {}", usage(name, command_arg));
                return ReplOutcome::Error;
            }
            _ => {}
        }
        match action {
            Action::Host(host) => ReplOutcome::Host(host, arg.to_owned()),
            Action::Builtin(Builtin::Help) => {
                self.print_help();
                ReplOutcome::Ok
            }
            Action::Builtin(Builtin::Quit) => ReplOutcome::Quit,
            Action::Builtin(Builtin::Type) => {
                match infer_expression_type(&self.accumulated_source, arg, &self.modules) {
                    Some(ty) => println!("{ty}"),
                    None => eprintln!("Could not infer type"),
                }
                ReplOutcome::Ok
            }
            Action::Builtin(Builtin::Time) => {
                let arg = arg.to_owned();
                self.eval(&arg, true)
            }
        }
    }

    /// Type-check, run and remember one piece of Python.
    fn eval(&mut self, code: &str, timed: bool) -> ReplOutcome<H> {
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
            return ReplOutcome::Error;
        }

        let start = timed.then(std::time::Instant::now);
        let scope = self.scope.as_ref().expect("scope is live").clone();
        let interp = self.interp.as_ref().expect("interpreter is live");
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

        let outcome = match ran {
            Err(status) => {
                self.exit_status = Some(status);
                return ReplOutcome::Quit;
            }
            Ok(true) => ReplOutcome::Ok,
            Ok(false) => ReplOutcome::Error,
        };

        if matches!(outcome, ReplOutcome::Ok) {
            self.append_source(&code);
            if let Some(start) = start {
                println!("Time: {}", format_elapsed(start.elapsed()));
            }
        }

        outcome
    }

    /// The commands, as the table has them.
    fn print_help(&self) {
        let width = self
            .commands
            .iter()
            .map(|c| c.usage().len())
            .max()
            .unwrap_or(0);
        println!("Commands:");
        for c in &self.commands {
            println!("  {:<width$}  {}", c.usage(), c.help);
        }
    }
}

impl<H> Drop for Repl<H> {
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

/// The page's REPL, with `source` already run and its doctests with it.
///
/// A file that raises still opens the session, so that the student can look
/// at what the definitions above the error left behind.
pub fn repl_new(source: &str, level: Level) -> Box<Repl> {
    // The page renders the ANSI codes, so its diagnostics keep them.
    let mut repl = Repl::new(level, true);
    if !source.trim().is_empty() && repl.load(source, "user.py", None, Vec::new()).is_ok() {
        let _ = repl.run_doctests();
    }
    Box::new(repl)
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
pub fn repl_run(repl: &mut Repl, code: &str) -> u32 {
    match repl.run(code) {
        ReplOutcome::Ok => REPL_OK,
        ReplOutcome::Error => REPL_ERROR,
        ReplOutcome::Quit => REPL_QUIT,
        // The page registers no commands of its own, and an `Infallible`
        // cannot be built, so there is nothing here to answer.
        ReplOutcome::Host(host, _) => match host {},
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
            let _repl = repl_new(source, Level::Full);
        }

        assert!(
            REPL_ATEXIT_RAN.load(Ordering::SeqCst),
            "expected atexit handler to run on repl drop"
        );
    }
}
