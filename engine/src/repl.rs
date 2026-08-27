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

/// What a command takes after its name.
#[derive(Clone, Copy)]
enum Arg {
    /// Nothing.
    None,
    /// Python, which is also what says whether the input is finished, so
    /// `:time [1,` waits for the rest of the list.
    Expr,
    /// An optional word, which the help shows exactly as it stands here.
    Word(&'static str),
}

/// Who answers a command: the REPL itself, or the frontend that registered
/// it, which names it with a value of its own `H`.
#[derive(Clone, Copy)]
enum Action<H> {
    Builtin(Builtin),
    Host(H),
}

#[derive(Clone, Copy)]
enum Builtin {
    Help,
    Quit,
    Type,
    Time,
}

/// The commands every frontend has. A terminal adds its own with
/// [`ReplSession::add`].
const BUILTINS: &[(&str, Arg, &str, Builtin)] = &[
    (":help", Arg::None, "Show this help", Builtin::Help),
    (":quit", Arg::None, "Exit the REPL", Builtin::Quit),
    (
        ":type",
        Arg::Expr,
        "Show the type of an expression",
        Builtin::Type,
    ),
    (
        ":time",
        Arg::Expr,
        "Evaluate and show execution time",
        Builtin::Time,
    ),
];

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

/// The name of the command at the head of the input and what follows it, or
/// none for Python, which never starts with `:`.
fn split(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim();
    if !trimmed.starts_with(':') {
        return None;
    }
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let (name, arg) = trimmed.split_at(end);
    Some((name, arg.trim_start()))
}

/// The REPL state that does not need the interpreter: what has run so far,
/// the level it is checked at, and whether its errors are coloured.
///
/// A frontend that already owns a VM -- the CLI, whose line editor borrows it
/// -- keeps one of these and calls [`ReplSession::run`] for every input.
pub struct ReplSession<H = Infallible> {
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
}

/// What running one input did.
pub enum ReplOutcome<H = Infallible> {
    /// The input ran.
    Ok,
    /// The input did not type-check, or it raised.
    Error,
    /// The session is over: `:quit`, or `exit()` / `quit()`, which raise a
    /// SystemExit saying with which status the process should leave.
    Quit(Option<vm::builtins::PyBaseExceptionRef>),
    /// A command the frontend registered, and what followed its name. Only
    /// the frontend knows what to do with it. The frontend names its commands
    /// with an enum, so leaving one unanswered is a compile error.
    Host(H, String),
}

impl<H: Copy> ReplSession<H> {
    /// A session that starts as if `accumulated_source` had already run, with
    /// `modules` the first-party files it imports.
    pub fn new(
        accumulated_source: String,
        level: Level,
        use_color: bool,
        modules: Vec<(String, String)>,
    ) -> Self {
        ReplSession {
            accumulated_source,
            level,
            use_color,
            modules,
            commands: BUILTINS
                .iter()
                .map(|&(name, arg, help, builtin)| Command {
                    name,
                    arg,
                    help,
                    action: Action::Builtin(builtin),
                })
                .collect(),
        }
    }

    /// Declare a command of the frontend's own, under the name `host` it
    /// knows it by. `word` is what the help shows after the name, for one
    /// that takes a word. The session checks it and completes it like any
    /// other, and hands it back as [`ReplOutcome::Host`] to be answered.
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

    /// The commands, each with the space that opens what it takes. What the
    /// page and the terminal offer when the line so far is a colon.
    pub fn command_completions(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .commands
            .iter()
            .map(|c| match c.arg {
                Arg::None => c.name.to_owned(),
                Arg::Expr | Arg::Word(_) => format!("{} ", c.name),
            })
            .collect();
        names.sort();
        names
    }

    /// The accumulated source of all successfully type-checked and executed inputs.
    pub fn accumulated_source(&self) -> &str {
        &self.accumulated_source
    }

    /// The first-party files the accumulated source imports.
    pub fn modules(&self) -> &[(String, String)] {
        &self.modules
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

    /// Execute one REPL input in `scope`: a command, or Python.
    ///
    /// Python is type-checked against everything that ran before it, and only
    /// joins the accumulated source once it has run.
    pub fn run(
        &mut self,
        vm: &vm::VirtualMachine,
        scope: vm::scope::Scope,
        code: &str,
    ) -> ReplOutcome<H> {
        let Some((name, arg)) = split(code) else {
            return self.eval(vm, scope, code, false);
        };
        // Copied out of the table so the command can be answered with the
        // session, which the table lives in, borrowed again.
        let Some((command_name, command_arg, action)) = self
            .commands
            .iter()
            .find(|c| c.name == name)
            .map(|c| (c.name, c.arg, c.action))
        else {
            eprintln!("Unknown command {name}. Type :help to see the commands.");
            return ReplOutcome::Error;
        };
        match (command_arg, arg.is_empty()) {
            (Arg::None, false) | (Arg::Expr, true) => {
                eprintln!("Usage: {}", usage(command_name, command_arg));
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
            Action::Builtin(Builtin::Quit) => ReplOutcome::Quit(None),
            Action::Builtin(Builtin::Type) => {
                match infer_expression_type(&self.accumulated_source, arg, &self.modules) {
                    Some(ty) => println!("{ty}"),
                    None => eprintln!("Could not infer type"),
                }
                ReplOutcome::Ok
            }
            Action::Builtin(Builtin::Time) => {
                let expr = arg.to_owned();
                self.eval(vm, scope, &expr, true)
            }
        }
    }

    /// Type-check, run and remember one piece of Python.
    fn eval(
        &mut self,
        vm: &vm::VirtualMachine,
        scope: vm::scope::Scope,
        code: &str,
        timed: bool,
    ) -> ReplOutcome<H> {
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
        let outcome = match vm
            .compile(&code, vm::compiler::Mode::Single, "<stdin>")
            .map_err(|err| err.into_pyexception(vm, Some(&code)))
            .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
        {
            Ok(_) => ReplOutcome::Ok,
            Err(exc) => {
                if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                    ReplOutcome::Quit(Some(exc))
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

    /// What the Tab key should do at `cursor_pos`, over this session's names
    /// and its commands.
    pub fn tab_action(&self, text: &str, cursor_pos: usize) -> crate::completion::TabAction {
        let commands = self.session.command_completions();
        self.with_vm(|vm, globals| {
            crate::completion::tab_action(vm, globals, text, cursor_pos, &commands)
        })
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
        // The page renders the ANSI codes, so its diagnostics keep them,
        // and it has one file, so there is nothing to import.
        session: ReplSession::new(accumulated_source, level, true, Vec::new()),
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
        // The page registers no commands of its own, and an `Infallible`
        // cannot be built, so there is nothing here to answer.
        ReplOutcome::Host(host, _) => match host {},
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
