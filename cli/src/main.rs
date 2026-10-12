#[cfg(target_arch = "wasm32")]
compile_error!("spython CLI cannot be compiled for wasm32; use spython-wasm instead");

use bpaf::{Bpaf, Parser};
mod config;
mod repl_reader;

use repl_reader::{ReplReader, Theme};

use engine::shell::welcome_message;
use engine::{
    Failed, Level, Loaded, Repl, Shell, SpythonError, Status, check, format, run, show_error,
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

/// Teaching level (0-5). The parser turns down a level nobody teaches, where
/// it turns down every other bad argument.
fn level_arg() -> impl bpaf::Parser<Level> {
    bpaf::short('l')
        .long("level")
        .help("Teaching level (0-5): 0=functions, 1=selection, 2=user types, 3=repetition, 4=classes, 5=full")
        .argument::<u8>("LEVEL")
        .parse(|n| Level::from_u8(n).ok_or(format!("invalid level {n}: must be 0-5")))
}

#[derive(Debug, Clone, Bpaf)]
enum Command {
    /// Start an interactive Python REPL (default)
    #[bpaf(command)]
    Repl {
        #[bpaf(external(level_arg), fallback(Level::Full))]
        level: Level,
        /// Optional Python file to execute before the REPL starts
        #[bpaf(positional("FILE"))]
        file: Option<PathBuf>,
    },
    /// Run a Python script
    #[bpaf(command)]
    Run {
        #[bpaf(external(level_arg), fallback(Level::Full))]
        level: Level,
        /// Python script to run
        #[bpaf(positional("FILE"))]
        file: PathBuf,
    },
    /// Check the types of a Python file without running it
    #[bpaf(command)]
    Check {
        #[bpaf(external(level_arg), fallback(Level::Full))]
        level: Level,
        /// Python file to check
        #[bpaf(positional("FILE"))]
        file: PathBuf,
    },
    /// Run doctests from the specified Python files
    #[bpaf(command)]
    Test {
        /// Show all test attempts, not just failures
        #[bpaf(short, long)]
        verbose: bool,
        #[bpaf(external(level_arg), fallback(Level::Full))]
        level: Level,
        /// Python files to run doctests from
        #[bpaf(
            positional("FILE"),
            some("expected `FILE`, pass `--help` for usage information")
        )]
        files: Vec<PathBuf>,
    },
    /// Format Python files (reads stdin if no path is given)
    #[bpaf(command)]
    Format {
        /// Check if the files are formatted without changing them
        #[bpaf(long)]
        check: bool,
        /// Files or directories to format (directories are searched recursively)
        #[bpaf(positional("PATH"), many)]
        paths: Vec<PathBuf>,
    },
    /// Show help information
    #[bpaf(command)]
    Help {
        /// Command to show the help of
        #[bpaf(positional("COMMAND"), optional)]
        command: Option<String>,
    },
}

/// The parser, with the two shorthands the students use most as alternatives
/// to a named subcommand: a leading `FILE` is a `run`, and bare options with
/// no file are a `repl`. `None` is the bare `spython`, which is a `repl` too.
fn cli() -> bpaf::OptionParser<Option<Command>> {
    let level = level_arg().fallback(Level::Full);
    let file = bpaf::positional::<PathBuf>("FILE");
    let file_as_run = bpaf::construct!(Command::Run { level, file });

    let level = level_arg().fallback(Level::Full);
    let file = bpaf::pure(None);
    let options_as_repl = bpaf::construct!(Command::Repl { level, file });

    let command = bpaf::construct!([command(), file_as_run, options_as_repl]).optional();
    bpaf::construct!(command)
        .to_options()
        .version(engine::version_short().as_str())
        .descr("A student version of Python with type checking and teaching levels")
        .footer("Levels: 0=functions, 1=selection, 2=user types, 3=repetition, 4=classes, 5=full")
}

/// Print what `parser` answers to `args`, which ask it for its help.
fn show_help<T>(parser: bpaf::OptionParser<T>, args: &[&str]) -> ExitCode {
    let Err(err) = parser.run_inner(bpaf::Args::from(args).set_name("spython")) else {
        return ExitCode::SUCCESS;
    };
    err.print_message(80);
    if err.exit_code() == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    engine::panic::add_handler();

    // Hidden flag for testing the panic handler
    if std::env::args().any(|a| a == "--test-panic") {
        panic!("test panic");
    }

    // The name of the executable ends in .exe on Windows.
    let command = match cli().run_inner(bpaf::Args::current_args().set_name("spython")) {
        Ok(cmd) => cmd,
        Err(err) => {
            err.print_message(80);
            return if err.exit_code() == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };

    let result = match command.unwrap_or(Command::Repl {
        level: Level::Full,
        file: None,
    }) {
        Command::Help { command: None } => return show_help(cli(), &["--help"]),
        // The commands alone, without the shorthands, so that a name that is
        // not a command lists the commands and does not run as a file.
        Command::Help {
            command: Some(name),
        } => return show_help(crate::command().to_options(), &[&name, "--help"]),
        Command::Repl { file, level } => {
            run_interactive(file.as_deref(), level, config::load().theme)
        }
        Command::Run { file, level } => run::run_main(&file, level),
        Command::Check { file, level } => check::check_file(&file, level).map(drop),
        Command::Format { check, paths } => format::run(check, &paths),
        Command::Test {
            files,
            verbose,
            level,
        } => run::run_test(&files, level, verbose),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            show_error(&e);
            ExitCode::FAILURE
        }
    }
}

/// Start an interactive Python REPL.
///
/// If `file` is given, it is type-checked and executed first so its
/// definitions are available in the REPL (like `python -i file.py`).
fn run_interactive(file: Option<&Path>, level: Level, theme: Theme) -> Result<(), SpythonError> {
    // The file is read and checked before the session opens, so that a file
    // the checker turns down leaves no banner behind.
    let preload = match file {
        Some(path) => Some(run::check_and_read(path, level)?),
        None => None,
    };

    print!("{}", welcome_message(level));

    let mut repl = Repl::new(level, engine::color_stderr());
    repl.install_displayhook();
    if let Some(script) = preload {
        let loaded = repl.load(
            &script.source,
            &script.name,
            Some(&script.dir),
            script.modules,
        );
        // An `exit()` in the file ends the session, as it does at the prompt.
        match loaded {
            Ok(Loaded::Ran) => (),
            Ok(Loaded::Exited(0)) => return Ok(()),
            Ok(Loaded::Exited(_)) | Err(Failed) => return Err(SpythonError::ProgramFailed),
        }
    }

    let theme = Rc::new(Cell::new(theme));
    let mut shell = Shell::new(repl);
    shell.add(
        ":theme",
        Some("[light|dark]"),
        "Show or change the theme",
        theme_action(Rc::clone(&theme)),
    );
    let commands = shell.command_names();
    let shell = Rc::new(RefCell::new(shell));

    let reader = ReplReader::new(Rc::clone(&shell), commands, theme)
        .map_err(|err| SpythonError::Other(Box::new(err)))?;
    for input in reader {
        if shell.borrow_mut().run(&input) == Status::Quit {
            break;
        }
    }

    match shell.borrow().repl().exit_status() {
        None | Some(0) => Ok(()),
        Some(_) => Err(SpythonError::ProgramFailed),
    }
}

/// `:theme` on its own says which theme is on; with a name it switches to it.
fn theme_action(theme: Rc<Cell<Theme>>) -> impl FnMut(&mut Repl, &str) -> Result<(), String> {
    move |_, arg| {
        if arg.is_empty() {
            println!("{}", theme.get().name());
            return Ok(());
        }
        let Some(chosen) = Theme::parse(arg) else {
            return Err("Invalid theme: use 'light' or 'dark'".to_owned());
        };
        theme.set(chosen);
        println!("{}", chosen.name());
        config::save_theme(chosen);
        Ok(())
    }
}
