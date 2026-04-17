#[cfg(target_arch = "wasm32")]
compile_error!("spython CLI cannot be compiled for wasm32; use spython-wasm instead");

use bpaf::{Bpaf, Parser};
use ruff_db::diagnostic::Diagnostic;
use ruff_db::files::system_path_to_file;
use ruff_db::system::{OsSystem, SystemPathBuf};
use ruff_python_ast::name::Name;
mod config;
mod repl;

use engine::{
    BUILD_DATE, GIT_HASH, LIBS_VERSION, LONG_VERSION, Level, VERSION, annotation_check,
    collect_import_files, execute_source, format_source, new_interpreter, print_type_errors,
};
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ty_project::{Db, ProjectDatabase, ProjectMetadata};
use walkdir::WalkDir;

/// Errors that can occur during file checking and execution.
enum Error {
    /// Failed to resolve file paths
    FileResolution(String),
    /// Failed to build the ty database
    DatabaseBuild(String),
    /// Failed to read a file
    FileRead(std::io::Error),
    /// Failed to write a file
    FileWrite(std::io::Error),
    /// Script execution failed
    ScriptExecution,
    /// Type checking errors found
    TypeChecking(Box<ProjectDatabase>, Vec<Diagnostic>),
}

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
    /// Run doctests from the specified Python files
    #[bpaf(command)]
    Check {
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

    let cli_parser = cli();
    let command = cli_parser.run();

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
        Command::Format { paths } => run_format(&paths),
        Command::Check {
            files,
            verbose,
            level,
        } => match parse_level(level) {
            Some(l) => run_check(&files, verbose, l),
            None => return ExitCode::FAILURE,
        },
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            display_error(e);
            ExitCode::FAILURE
        }
    }
}

/// Display an error to the user.
fn display_error(error: Error) {
    match error {
        Error::FileResolution(msg) | Error::DatabaseBuild(msg) => {
            eprintln!("spython: {msg}");
        }
        Error::FileRead(e) => {
            eprintln!("spython: cannot read file: {e}");
        }
        Error::FileWrite(e) => {
            eprintln!("spython: cannot write file: {e}");
        }
        Error::ScriptExecution => {
            // Error already displayed by RustPython
        }
        Error::TypeChecking(db, diagnostics) => {
            print_type_errors(&db, &diagnostics, std::io::stderr().is_terminal());
        }
    }
}

/// Start an interactive Python REPL.
///
/// If `file` is given, it is type-checked and executed first so its
/// definitions are available in the REPL (like `python -i file.py`).
fn start_repl(file: Option<&Path>, level: Level) -> Result<(), Error> {
    if let Some(path) = file {
        type_check_file(path, level)?;
    }

    let preload = file
        .map(|path| -> Result<_, Error> {
            let source = std::fs::read_to_string(path).map_err(Error::FileRead)?;
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
            vm.run_string(scope.clone(), source, file_str.clone())
                .map(drop)?;
        }
        repl::run_repl(vm, scope, level)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(Error::ScriptExecution)
    }
}

/// Type-check a Python file: validates the path, builds the ty database,
/// runs the annotation checker, then runs ty's type checker.
fn type_check_file(file: &Path, level: Level) -> Result<(), Error> {
    let abs_file = dunce::canonicalize(file).map_err(|e| {
        Error::FileResolution(format!(
            "cannot resolve '{}' to an absolute path: {e}",
            file.display()
        ))
    })?;
    if !abs_file.is_file() {
        return Err(Error::FileResolution(format!(
            "'{}' is not a file",
            file.display()
        )));
    }

    let cwd = std::env::current_dir().map_err(|e| Error::DatabaseBuild(e.to_string()))?;
    let mut db = build_db(&cwd)?;

    let abs_sys = SystemPathBuf::from_path_buf(abs_file).map_err(|p| {
        Error::FileResolution(format!("'{}' contains non-Unicode characters", p.display()))
    })?;
    let main_file =
        system_path_to_file(&db, &abs_sys).map_err(|e| Error::FileResolution(e.to_string()))?;

    let mut files = HashSet::new();
    files.insert(main_file);
    collect_import_files(&db, main_file, &mut files);

    let sys_files: Vec<SystemPathBuf> = files
        .iter()
        .filter_map(|f| f.path(&db).as_system_path().map(|p| p.to_path_buf()))
        .collect();
    db.project().set_included_paths(&mut db, sys_files);

    let mut diagnostics = annotation_check(&db, level, false);
    // Filter out unresolved-import errors for the spython library module,
    // which is frozen into the binary and not visible to ty's resolver.
    diagnostics.extend(db.check().into_iter().filter(|d| {
        !(d.id().as_str() == "unresolved-import" && d.primary_message().contains("spython"))
    }));
    // Validate and type-check doctests inside each user file's docstrings.
    for f in &files {
        let is_spython_library = f
            .path(&db)
            .as_system_path()
            .is_some_and(|p| p.as_str().contains("/Lib/spython/"));
        if !is_spython_library {
            diagnostics.extend(engine::check_file_doctests(&db, *f, level));
        }
    }
    if !diagnostics.is_empty() {
        return Err(Error::TypeChecking(Box::new(db), diagnostics));
    }

    Ok(())
}

/// Run a Python script with type checking enabled.
fn run_checked(file: &Path, level: Level) -> Result<(), Error> {
    type_check_file(file, level)?;

    let source = std::fs::read_to_string(file).map_err(Error::FileRead)?;
    let file_str = file.to_string_lossy().into_owned();
    let parent_dir = file
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_owned();

    if execute_source(&source, &file_str, &parent_dir) {
        Ok(())
    } else {
        Err(Error::ScriptExecution)
    }
}

/// Format Python files in the given paths, recursing into directories.
fn run_format(paths: &[PathBuf]) -> Result<(), Error> {
    for path in paths {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        {
            let file_path = entry.path();
            let source = std::fs::read_to_string(file_path).map_err(Error::FileRead)?;
            let formatted = match format_source(&source) {
                Ok(formatted) => formatted,
                Err(e) => {
                    eprintln!("spython: cannot format '{}': {e}", file_path.display());
                    continue;
                }
            };
            if formatted != source {
                std::fs::write(file_path, formatted).map_err(Error::FileWrite)?;
            }
        }
    }
    Ok(())
}

/// Run doctests for the given files, ignoring paths that are not files.
fn run_check(files: &[PathBuf], verbose: bool, level: Level) -> Result<(), Error> {
    let valid_files: Vec<&PathBuf> = files.iter().filter(|f| f.is_file()).collect();

    if valid_files.is_empty() {
        return Ok(());
    }

    for file in &valid_files {
        type_check_file(file, level)?;
    }

    let py_verbose = if verbose { "True" } else { "False" };
    let doctest_runner = engine::DOCTEST_RUNNER;
    let mut script = format!(
        "{doctest_runner}\n\
         import types, sys\n\
         def _run(path):\n\
         \x20   mod = types.ModuleType('__doctest__')\n\
         \x20   mod.__file__ = path\n\
         \x20   with open(path) as f:\n\
         \x20       exec(compile(f.read(), path, 'exec'), vars(mod))\n\
         \x20   return run_doctests(mod, verbose={py_verbose})\n\
         total = 0\n",
    );
    let print_names = valid_files.len() > 1;
    for file in &valid_files {
        let path = file.to_string_lossy();
        let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        if print_names {
            script.push_str(&format!("print(\"{escaped}\")\n"));
        }
        script.push_str(&format!("total += _run(\"{escaped}\")\n"));
    }
    script.push_str("sys.exit(total)\n");

    if execute_source(&script, "<check>", ".") {
        Ok(())
    } else {
        Err(Error::ScriptExecution)
    }
}

/// Build a ty ProjectDatabase for the given file.
fn build_db(cwd: &Path) -> Result<ProjectDatabase, Error> {
    let cwd_sys = SystemPathBuf::from_path_buf(cwd.to_path_buf()).map_err(|_| {
        Error::DatabaseBuild("current directory contains non-Unicode characters".to_string())
    })?;

    let system = OsSystem::new(&cwd_sys);
    let metadata = ProjectMetadata::new(Name::new("spython"), cwd_sys);

    ProjectDatabase::new(metadata, system).map_err(|e| Error::DatabaseBuild(e.to_string()))
}
