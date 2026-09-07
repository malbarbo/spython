#[cfg(target_arch = "wasm32")]
compile_error!("spython CLI cannot be compiled for wasm32; use spython-wasm instead");

use bpaf::{Bpaf, Parser};
use ruff_db::files::system_path_to_file;
use ruff_db::system::SystemPathBuf;
mod config;
mod overlay_system;
mod repl;

use overlay_system::OverlaySystem;

use engine::{
    BUILD_DATE, GIT_HASH, LIBS_VERSION, LONG_VERSION, Level, SpythonError, TypeErrors, VERSION,
    annotation_check, collect_import_files, execute_source, format_source, make_project_metadata,
    new_interpreter, print_type_errors, show_error, spython_lib,
};
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ty_project::{Db, ProjectDatabase};
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
        .version(LONG_VERSION)
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
            repl::set_theme(cfg.theme == "light");
            start_repl(file.as_deref(), l)
        }
        Command::Run { file, level } => match parse_level(level) {
            Some(l) => run_checked(&file, l),
            None => return ExitCode::FAILURE,
        },
        Command::Check { file, level } => match parse_level(level) {
            Some(l) => type_check_file(&file, l).map(drop),
            None => return ExitCode::FAILURE,
        },
        Command::Format { paths } => run_format(&paths),
        Command::Test {
            files,
            verbose,
            level,
        } => match parse_level(level) {
            Some(l) => run_test(&files, verbose, l),
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
fn start_repl(file: Option<&Path>, level: Level) -> Result<(), SpythonError> {
    let modules = match file {
        Some(path) => type_check_file(path, level)?,
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
        repl::run_repl(vm, scope, accumulated, level, modules)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(SpythonError::ProgramFailed)
    }
}

/// Type-check a Python file: validates the path, builds the ty database,
/// runs the annotation checker, then runs ty's type checker.
///
/// Returns the first-party files it imports, each a path relative to its own
/// directory and its text, for a REPL that has to keep checking against them.
fn type_check_file(file: &Path, level: Level) -> Result<Vec<(String, String)>, SpythonError> {
    let abs_file = dunce::canonicalize(file).map_err(|err| SpythonError::FileRead {
        path: file.to_path_buf(),
        err,
    })?;
    if !abs_file.is_file() {
        return Err(SpythonError::PathIsNotAFile {
            path: file.to_path_buf(),
        });
    }

    let abs_sys = SystemPathBuf::from_path_buf(abs_file)
        .map_err(|path| SpythonError::NonUtf8Path { path })?;
    // The file's own directory, not the process's: `import helpers` looks
    // beside the file that wrote it, which is also where the interpreter
    // looks when it runs (`sys.path[0]`). Rooting the check anywhere else
    // makes `spython test dir/main.py` disagree with `cd dir` for no reason
    // the student can see.
    let root = abs_sys.parent().unwrap_or(&abs_sys).to_path_buf();
    let mut db = build_db(root.as_std_path())?;
    let main_file =
        system_path_to_file(&db, &abs_sys).map_err(|e| SpythonError::Database(e.to_string()))?;

    let mut files = HashSet::new();
    files.insert(main_file);
    collect_import_files(&db, main_file, &mut files);

    let sys_files: Vec<SystemPathBuf> = files
        .iter()
        .filter_map(|f| f.path(&db).as_system_path().map(|p| p.to_path_buf()))
        .collect();
    db.project().set_included_paths(&mut db, sys_files);

    let mut diagnostics = annotation_check(&db, level, false);
    diagnostics.extend(db.check());
    // Validate and type-check doctests inside each user file's docstrings.
    // Sort to stabilize diagnostic order across platforms — `files` is a
    // `HashSet`, whose iteration order varies.
    let mut user_files: Vec<_> = files
        .iter()
        .copied()
        .filter(|f| {
            !f.path(&db)
                .as_system_path()
                .is_some_and(spython_lib::is_lib_path)
        })
        .collect();
    user_files.sort_by_key(|f| f.path(&db).as_str().to_owned());
    for f in &user_files {
        diagnostics.extend(engine::check_file_doctests(&db, *f, level));
    }
    // An error stops the file; a warning is printed and the file goes on. The
    // in-memory check the page runs has always drawn the line here, and this is
    // where the one over the filesystem draws it too.
    if diagnostics
        .iter()
        .any(|d| d.severity() == ruff_db::diagnostic::Severity::Error)
    {
        return Err(SpythonError::TypeErrors(Box::new(TypeErrors {
            db,
            diagnostics,
        })));
    }
    if !diagnostics.is_empty() {
        print_type_errors(&db, &diagnostics, std::io::stderr().is_terminal());
    }

    Ok(engine::modules_beside(&db, main_file, user_files))
}

/// Run a Python script with type checking enabled.
fn run_checked(file: &Path, level: Level) -> Result<(), SpythonError> {
    type_check_file(file, level)?;

    let source = read_file(file)?;
    let file_str = file.to_string_lossy().into_owned();
    let parent_dir = file
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_owned();

    if execute_source(&source, &file_str, &parent_dir) {
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

/// A path as a Python string literal's body.
fn escape(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Run doctests for the given files.
fn run_test(files: &[PathBuf], verbose: bool, level: Level) -> Result<(), SpythonError> {
    if files.is_empty() {
        return Ok(());
    }

    // A file that is not there is an error, the way it is for `run`. Silently
    // reporting no failures for a name the student mistyped is worse than
    // saying the name is wrong.
    for file in files {
        type_check_file(file, level)?;
    }

    let py_verbose = if verbose { "True" } else { "False" };
    let doctest_runner = engine::DOCTEST_RUNNER;
    let mut script = format!(
        "{doctest_runner}\n\
         import types, sys\n\
         def _run(path, root):\n\
         \x20   sys.path.insert(0, root)\n\
         \x20   mod = types.ModuleType('__doctest__')\n\
         \x20   mod.__file__ = path\n\
         \x20   with open(path) as f:\n\
         \x20       exec(compile(f.read(), path, 'exec'), vars(mod))\n\
         \x20   return run_doctests(mod, verbose={py_verbose})\n\
         total = 0\n",
    );
    let print_names = files.len() > 1;
    for file in files {
        let path = file.to_string_lossy();
        let escaped = escape(&path);
        // The doctests import the way the file they came from does, so they
        // run with its own directory on the path -- the same directory the
        // static check resolved them against.
        let root = escape(&file.parent().unwrap_or(Path::new(".")).to_string_lossy());
        let root = if root.is_empty() {
            ".".to_owned()
        } else {
            root
        };
        if print_names {
            script.push_str(&format!("print(\"{escaped}\", flush=True)\n"));
        }
        script.push_str(&format!("total += _run(\"{escaped}\", \"{root}\")\n"));
    }
    script.push_str("sys.exit(total)\n");

    if execute_source(&script, "<test>", ".") {
        Ok(())
    } else {
        Err(SpythonError::TestsFailed)
    }
}

/// Build a ty ProjectDatabase for the given file.
fn build_db(cwd: &Path) -> Result<ProjectDatabase, SpythonError> {
    let cwd_sys = SystemPathBuf::from_path_buf(cwd.to_path_buf())
        .map_err(|path| SpythonError::NonUtf8Path { path })?;

    let (system, lib_root) = OverlaySystem::new(&cwd_sys);
    let metadata = make_project_metadata(cwd_sys, vec![lib_root]);

    ProjectDatabase::fallible(metadata, system).map_err(|e| SpythonError::Database(e.to_string()))
}

/// Read a file, naming it in whatever the filesystem says about the read.
fn read_file(path: &Path) -> Result<String, SpythonError> {
    std::fs::read_to_string(path).map_err(|err| SpythonError::FileRead {
        path: path.to_path_buf(),
        err,
    })
}
