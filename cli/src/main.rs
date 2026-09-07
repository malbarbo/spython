#[cfg(target_arch = "wasm32")]
compile_error!("spython CLI cannot be compiled for wasm32; use spython-wasm instead");

use bpaf::{Bpaf, Parser};
mod config;
mod repl;

use repl::Theme;

use engine::{
    BUILD_DATE, GIT_HASH, LIBS_VERSION, Level, Repl, SpythonError, VERSION, check, format_source,
    read_file, run, show_error,
};
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

/// Teaching level (0-5)
fn level_arg() -> impl bpaf::Parser<u8> {
    bpaf::short('l')
        .long("level")
        .help("Teaching level (0-5): 0=functions, 1=selection, 2=user types, 3=repetition, 4=classes, 5=full")
        .argument::<u8>("LEVEL")
}

#[derive(Debug, Clone, Bpaf)]
enum Command {
    /// Start an interactive Python REPL (default)
    #[bpaf(command)]
    Repl {
        #[bpaf(external(level_arg), optional)]
        level: Option<u8>,
        /// Optional Python file to execute before the REPL starts
        #[bpaf(positional("FILE"))]
        file: Option<PathBuf>,
    },
    /// Run a Python script
    #[bpaf(command)]
    Run {
        #[bpaf(external(level_arg), fallback(0))]
        level: u8,
        /// Python script to run
        #[bpaf(positional("FILE"))]
        file: PathBuf,
    },
    /// Check the types of a Python file without running it
    #[bpaf(command)]
    Check {
        #[bpaf(external(level_arg), fallback(0))]
        level: u8,
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
        #[bpaf(external(level_arg), fallback(0))]
        level: u8,
        /// Python files to run doctests from
        #[bpaf(positional("FILE"), many)]
        files: Vec<PathBuf>,
    },
    /// Format Python files
    #[bpaf(command)]
    Format {
        /// Files or directories to format (directories are searched recursively)
        #[bpaf(positional("PATH"), many)]
        paths: Vec<PathBuf>,
    },
    /// Show help information
    #[bpaf(command)]
    Help,
}

/// The parser, with the two shorthands the students use most as alternatives
/// to a named subcommand: a leading `FILE` is a `run`, and bare options with
/// no file are a `repl`. `None` is the bare `spython`, which is a `repl` too.
fn cli() -> bpaf::OptionParser<Option<Command>> {
    let level = level_arg().fallback(0);
    let file = bpaf::positional::<PathBuf>("FILE");
    let file_as_run = bpaf::construct!(Command::Run { level, file });

    let level = level_arg().map(Some);
    let file = bpaf::pure(None);
    let options_as_repl = bpaf::construct!(Command::Repl { level, file });

    let command = bpaf::construct!([command(), file_as_run, options_as_repl]).optional();
    bpaf::construct!(command)
        .to_options()
        .version(engine::version_short().as_str())
        .descr("A student version of Python with type checking and teaching levels")
        .footer("Levels: 0=functions, 1=selection, 2=user types, 3=repetition, 4=classes, 5=full")
}

fn parse_level(level: u8) -> Option<Level> {
    Level::from_u8(level).or_else(|| {
        eprintln!("Invalid level {level}: must be 0-5");
        None
    })
}

fn main() -> ExitCode {
    engine::panic::add_handler();

    // Hidden flag for testing the panic handler
    if std::env::args().any(|a| a == "--test-panic") {
        panic!("test panic");
    }

    let command = match cli().run_inner(bpaf::Args::current_args()) {
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
        level: None,
        file: None,
    }) {
        Command::Help => {
            if let Err(err) = cli().run_inner(bpaf::Args::from(&["--help"])) {
                err.print_message(80);
            }
            return ExitCode::SUCCESS;
        }
        Command::Repl { file, level } => {
            let cfg = config::load();
            let l = match level {
                Some(n) => match parse_level(n) {
                    Some(l) => l,
                    None => return ExitCode::FAILURE,
                },
                None => cfg.level,
            };
            start_repl(file.as_deref(), l, cfg.theme)
        }
        Command::Run { file, level } => match parse_level(level) {
            Some(l) => run::run_main(&file, l),
            None => return ExitCode::FAILURE,
        },
        Command::Check { file, level } => match parse_level(level) {
            Some(l) => run::run_check(&file, l),
            None => return ExitCode::FAILURE,
        },
        Command::Format { paths } => run_format(&paths),
        Command::Test {
            files,
            verbose,
            level,
        } => match parse_level(level) {
            Some(l) => run::run_test(&files, l, verbose),
            None => return ExitCode::FAILURE,
        },
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
fn start_repl(file: Option<&Path>, level: Level, theme: Theme) -> Result<(), SpythonError> {
    // The file is read and checked before the session opens, so that a file
    // the checker turns down leaves no banner behind.
    let preload = match file {
        Some(path) => {
            let modules = check::check_file(path, level)?;
            let source = read_file(path)?;
            let name = path.to_string_lossy().into_owned();
            let dir = path
                .parent()
                .and_then(Path::to_str)
                .unwrap_or(".")
                .to_owned();
            Some((source, name, dir, modules))
        }
        None => None,
    };

    println!(
        "spython level {level} - {VERSION} ({BUILD_DATE}, {GIT_HASH})\n\
         Using {LIBS_VERSION}.\n\
         Type :help for commands, :quit or ctrl-d to exit."
    );

    // A terminal is what wants the colour; a redirected stderr keeps the
    // codes out of the file it is being read into.
    let mut repl = Repl::new(level, std::io::stderr().is_terminal());
    if let Some((source, name, dir, modules)) = preload
        && repl.load(&source, &name, Some(&dir), modules).is_err()
    {
        return Err(SpythonError::ProgramFailed);
    }

    match repl::run_repl(repl, theme) {
        None | Some(0) => Ok(()),
        Some(_) => Err(SpythonError::ProgramFailed),
    }
}

/// Format Python files in the given paths, recursing into directories.
fn run_format(paths: &[PathBuf]) -> Result<(), SpythonError> {
    for path in paths {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        {
            let file_path = entry.path();
            let source = read_file(file_path)?;
            let formatted = match format_source(&source) {
                Ok(formatted) => formatted,
                // One file the formatter turns down does not stop the others.
                Err(e) => {
                    show_error(&SpythonError::Format {
                        path: file_path.to_path_buf(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            if formatted != source {
                std::fs::write(file_path, formatted).map_err(|err| SpythonError::FileWrite {
                    path: file_path.to_path_buf(),
                    err,
                })?;
            }
        }
    }
    Ok(())
}
