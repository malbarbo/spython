#[cfg(target_arch = "wasm32")]
compile_error!("spython CLI cannot be compiled for wasm32; use spython-wasm instead");

use bpaf::{Bpaf, Parser};
mod config;
mod repl;

use repl::Theme;

use engine::{
    BUILD_DATE, GIT_HASH, LIBS_VERSION, Level, SpythonError, VERSION, check, format_source,
    new_interpreter, read_file, run, show_error,
};
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

fn cli() -> bpaf::OptionParser<Option<Command>> {
    let command = command().optional();
    bpaf::construct!(command)
        .to_options()
        .version(engine::version_short().as_str())
        .descr("A student version of Python with type checking and teaching levels")
        .footer("Levels: 0=functions, 1=selection, 2=user types, 3=repetition, 4=classes, 5=full")
}

/// Names of the explicit subcommands. Used to decide whether a bare leading
/// positional should be treated as a file to run.
const COMMANDS: &[&str] = &["repl", "run", "check", "test", "format", "help"];

/// Whether `a` is a top-level help or version request that must reach the
/// normal parser (rather than being routed into an injected subcommand).
fn is_help_or_version(a: &str) -> bool {
    matches!(a, "-h" | "--help" | "-V" | "--version")
}

/// The first positional argument and whether it was forced positional by a
/// `--` separator. A `--`-forced token is definitely a file (never a
/// subcommand); a bare token may be a subcommand name. The value consumed by
/// `-l`/`--level` is skipped so it is not mistaken for a positional.
fn first_positional(args: &[String]) -> Option<(&str, bool)> {
    let mut i = 0;
    while let Some(a) = args.get(i) {
        if a == "--" {
            return args.get(i + 1).map(|s| (s.as_str(), true));
        }
        if a == "-l" || a == "--level" {
            i += 2; // skip the option and its separate value
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        return Some((a, false));
    }
    None
}

/// Inject a default subcommand so a leading `FILE` or bare options Just Work:
///
/// - `spython file.py [--level N]` → `run file.py [--level N]` (a token forced
///   positional by `--` is always a file, even if it matches a subcommand name)
/// - `spython [--level N]` with no file → `repl [--level N]`
/// - `spython --help`/`--version` and explicit subcommands are left untouched.
fn effective_args() -> Vec<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let inject = match first_positional(&args) {
        Some((_, true)) => Some("run"),
        Some((tok, false)) if !COMMANDS.contains(&tok) => Some("run"),
        Some(_) => None, // explicit subcommand
        None => {
            // No file or subcommand. Options other than --help/--version are
            // meant for an interactive session (e.g. `spython -l 2`).
            if args.is_empty() || args.iter().any(|a| is_help_or_version(a)) {
                None
            } else {
                Some("repl")
            }
        }
    };
    match inject {
        Some(cmd) => {
            let mut with_cmd = Vec::with_capacity(args.len() + 1);
            with_cmd.push(cmd.to_string());
            with_cmd.extend(args);
            with_cmd
        }
        None => args,
    }
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

    let args = effective_args();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let command = match cli().run_inner(bpaf::Args::from(arg_refs.as_slice())) {
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
    let modules = match file {
        Some(path) => check::check_file(path, level)?,
        None => Vec::new(),
    };

    let preload = file
        .map(|path| -> Result<_, SpythonError> {
            let source = read_file(path)?;
            let file_str = path.to_string_lossy().into_owned();
            let parent_dir = path
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".")
                .to_owned();
            Ok((source, file_str, parent_dir))
        })
        .transpose()?;

    let interp = new_interpreter();
    let code = interp.run(|vm| {
        let scope = vm.new_scope_with_main()?;
        println!(
            "spython level {level} - {VERSION} ({BUILD_DATE}, {GIT_HASH})\n\
             Using {LIBS_VERSION}.\n\
             Type :help for commands, :quit or ctrl-d to exit."
        );
        vm.sys_module.set_attr("ps1", vm.ctx.new_str(">>> "), vm)?;
        vm.sys_module.set_attr("ps2", vm.ctx.new_str("... "), vm)?;
        if let Some((source, file_str, parent_dir)) = &preload {
            vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
            vm.run_string(scope.clone(), source, file_str).map(drop)?;
        }
        // The file that ran is the context the next inputs are checked in,
        // together with whatever it imports.
        let accumulated = preload
            .as_ref()
            .map_or_else(String::new, |(source, ..)| source.clone());
        repl::run_repl(vm, scope, accumulated, level, theme, modules)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(SpythonError::ProgramFailed)
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
