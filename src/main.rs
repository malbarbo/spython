use clap::Parser;
use ruff_db::diagnostic::{Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{OsSystem, SystemPathBuf};
use ruff_python_ast::Stmt;
use ruff_python_ast::name::Name;
use ruff_python_formatter::{PyFormatOptions, format_module_source};
use rustpython::{InterpreterBuilder, InterpreterBuilderExt, run_shell};
use rustpython::vm::Settings;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ty_module_resolver::{ModuleName, resolve_module};
use ty_project::{Db, ProjectDatabase, ProjectMetadata};
use walkdir::WalkDir;

mod checker;
mod lints;

/// Errors that can occur during file checking and execution.
enum CheckError {
    /// Failed to resolve file paths
    FileResolution(String),
    /// Failed to build the ty database
    DatabaseBuild(String),
    /// Annotation errors found
    AnnotationErrors(ProjectDatabase, Vec<Diagnostic>),
    /// Type checking errors found
    TypeErrors(ProjectDatabase, Vec<Diagnostic>),
    /// Failed to read a file
    FileRead(std::io::Error),
    /// Failed to write a file
    FileWrite(std::io::Error),
    /// Script execution failed
    ScriptExecution,
}

/// spython: A Python interpreter with integrated type checking for students
#[derive(Parser)]
#[clap(name = "spython", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start an interactive Python REPL (default)
    Repl {
        /// Optional Python file to execute before the REPL starts
        file: Option<PathBuf>,
    },
    /// Run a Python script
    Run {
        /// Python script to run
        file: PathBuf,
    },
    /// Format Python files
    Format {
        /// Files or directories to format (directories are searched recursively)
        paths: Vec<PathBuf>,
    },
    /// Run doctests from the specified Python files
    Check {
        /// Python files to run doctests from
        files: Vec<PathBuf>,
        /// Show all test attempts, not just failures
        #[arg(short, long)]
        verbose: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command.unwrap_or(Commands::Repl { file: None }) {
        Commands::Repl { file } => start_repl(file.as_deref()),
        Commands::Run { file } => run_checked(&file),
        Commands::Format { paths } => run_format(&paths),
        Commands::Check { files, verbose } => run_check(&files, verbose),
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
fn display_error(error: CheckError) {
    match error {
        CheckError::FileResolution(msg) | CheckError::DatabaseBuild(msg) => {
            eprintln!("spython: {msg}");
        }
        CheckError::AnnotationErrors(db, diagnostics) => {
            display_diagnostics(&db, &diagnostics, "annotation error");
        }
        CheckError::TypeErrors(db, diagnostics) => {
            display_diagnostics(&db, &diagnostics, "diagnostic");
        }
        CheckError::FileRead(e) => {
            eprintln!("spython: cannot read file: {e}");
        }
        CheckError::FileWrite(e) => {
            eprintln!("spython: cannot write file: {e}");
        }
        CheckError::ScriptExecution => {
            // Error already displayed by RustPython
        }
    }
}

fn new_interpreter() -> rustpython::vm::Interpreter {
    let mut settings = Settings::default();
    settings.write_bytecode = false;
    InterpreterBuilder::new()
        .settings(settings)
        .init_stdlib()
        .interpreter()
}

/// Start an interactive Python REPL.
///
/// If `file` is given, it is type-checked and executed first so its
/// definitions are available in the REPL (like `python -i file.py`).
fn start_repl(file: Option<&Path>) -> Result<(), CheckError> {
    if let Some(path) = file {
        type_check_file(path)?;
    }

    let preload: Option<(String, String, String)> = file
        .map(|path| -> Result<_, CheckError> {
            let source = std::fs::read_to_string(path).map_err(CheckError::FileRead)?;
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
        vm.sys_module.set_attr("ps1", vm.ctx.new_str("> "), vm)?;
        vm.sys_module.set_attr("ps2", vm.ctx.new_str("  "), vm)?;
        if let Some((source, file_str, parent_dir)) = &preload {
            vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
            vm.run_string(scope.clone(), source, file_str.clone()).map(drop)?;
        }
        run_shell(vm, scope)
    });
    if code == 0 {
        Ok(())
    } else {
        Err(CheckError::ScriptExecution)
    }
}

/// Type-check a Python file: validates the path, builds the ty database,
/// runs the annotation checker, then runs ty's type checker.
fn type_check_file(file: &Path) -> Result<(), CheckError> {
    let abs_file = std::fs::canonicalize(file).map_err(|e| {
        CheckError::FileResolution(format!(
            "cannot resolve '{}' to an absolute path: {e}",
            file.display()
        ))
    })?;
    if !abs_file.is_file() {
        return Err(CheckError::FileResolution(format!(
            "'{}' is not a file",
            file.display()
        )));
    }

    let cwd = std::env::current_dir().map_err(|e| CheckError::DatabaseBuild(e.to_string()))?;
    let mut db = build_db(&cwd)?;

    let abs_sys = SystemPathBuf::from_path_buf(abs_file).map_err(|p| {
        CheckError::FileResolution(format!(
            "'{}' contains non-Unicode characters",
            p.display()
        ))
    })?;
    let main_file = system_path_to_file(&db, &abs_sys)
        .map_err(|e| CheckError::FileResolution(e.to_string()))?;

    let mut local_files: HashSet<File> = HashSet::new();
    local_files.insert(main_file);
    collect_local_imports(&db, main_file, &mut local_files);

    let sys_files: Vec<SystemPathBuf> = local_files
        .iter()
        .filter_map(|f| f.path(&db).as_system_path().map(|p| p.to_path_buf()))
        .collect();
    db.project().set_included_paths(&mut db, sys_files);

    let annotation_diagnostics = annotation_check(&db);
    if !annotation_diagnostics.is_empty() {
        return Err(CheckError::AnnotationErrors(db, annotation_diagnostics));
    }

    let type_diagnostics = db.check();
    if !type_diagnostics.is_empty() {
        return Err(CheckError::TypeErrors(db, type_diagnostics));
    }

    Ok(())
}

/// Run a Python script with type checking enabled.
fn run_checked(file: &Path) -> Result<(), CheckError> {
    type_check_file(file)?;

    let source = std::fs::read_to_string(file).map_err(CheckError::FileRead)?;
    let file_str = file.to_string_lossy().into_owned();
    let parent_dir = file
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_owned();

    let interp = new_interpreter();
    let code = interp.run(move |vm| {
        let scope = vm.new_scope_with_main()?;
        vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
        vm.run_string(scope, &source, file_str).map(drop)
    });

    if code == 0 {
        Ok(())
    } else {
        Err(CheckError::ScriptExecution)
    }
}

/// Format Python files in the given paths, recursing into directories.
fn run_format(paths: &[PathBuf]) -> Result<(), CheckError> {
    for path in paths {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        {
            let file_path = entry.path();
            let source = std::fs::read_to_string(file_path).map_err(CheckError::FileRead)?;
            let options = PyFormatOptions::default();
            let formatted = match format_module_source(&source, options) {
                Ok(printed) => printed.into_code(),
                Err(e) => {
                    eprintln!("spython: cannot format '{}': {e}", file_path.display());
                    continue;
                }
            };
            if formatted != source {
                std::fs::write(file_path, formatted).map_err(CheckError::FileWrite)?;
            }
        }
    }
    Ok(())
}

/// Run doctests for the given files, ignoring paths that are not files.
fn run_check(files: &[PathBuf], verbose: bool) -> Result<(), CheckError> {
    // Use original paths (not canonicalized) so output stays relative when
    // the caller passes relative paths, making test snapshots portable.
    let valid_files: Vec<&PathBuf> = files.iter().filter(|f| f.is_file()).collect();

    if valid_files.is_empty() {
        return Ok(());
    }

    for file in &valid_files {
        type_check_file(file)?;
    }

    let py_verbose = if verbose { "True" } else { "False" };
    // Build a Python script that imports each file as a module and runs
    // doctest.testmod on it (same as `python -m doctest file.py`).
    let mut script = format!(
        "import doctest, sys, importlib.util\n\
         def _run(path):\n\
         \x20   spec = importlib.util.spec_from_file_location('__doctest__', path)\n\
         \x20   mod = importlib.util.module_from_spec(spec)\n\
         \x20   spec.loader.exec_module(mod)\n\
         \x20   return doctest.testmod(mod, verbose={py_verbose}).failed\n\
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

    let interp = new_interpreter();
    let code = interp.run(|vm| {
        let scope = vm.new_scope_with_main()?;
        vm.run_string(scope, &script, "<check>".to_owned()).map(drop)
    });

    if code == 0 {
        Ok(())
    } else {
        Err(CheckError::ScriptExecution)
    }
}

/// Recursively collect first-party files transitively imported by `file`.
///
/// Uses ty's module resolver so relative imports, packages, and dotted names
/// are all handled correctly.
fn collect_local_imports(db: &ProjectDatabase, file: File, seen: &mut HashSet<File>) {
    let parsed = parsed_module(db, file);
    let module = parsed.load(db);

    for stmt in module.suite() {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    if let Some(name) = ModuleName::new(alias.name.id.as_str()) {
                        visit_module(db, file, &name, seen);
                    }
                }
            }
            Stmt::ImportFrom(import_from) => {
                let module_str = import_from.module.as_deref();
                if let Ok(name) =
                    ModuleName::from_identifier_parts(db, file, module_str, import_from.level)
                {
                    visit_module(db, file, &name, seen);
                }
            }
            _ => {}
        }
    }
}

/// Resolve `name` relative to `importing_file` and, if it is a first-party
/// module not yet seen, add it to `seen` and recurse into its imports.
fn visit_module(
    db: &ProjectDatabase,
    importing_file: File,
    name: &ModuleName,
    seen: &mut HashSet<File>,
) {
    if let Some(module) = resolve_module(db, importing_file, name) {
        if module.search_path(db).is_some_and(|sp| sp.is_first_party()) {
            if let Some(mod_file) = module.file(db) {
                if seen.insert(mod_file) {
                    collect_local_imports(db, mod_file, seen);
                }
            }
        }
    }
}

/// Build a ty ProjectDatabase for the given file.
fn build_db(cwd: &Path) -> Result<ProjectDatabase, CheckError> {
    let cwd_sys = SystemPathBuf::from_path_buf(cwd.to_path_buf()).map_err(|_| {
        CheckError::DatabaseBuild("current directory contains non-Unicode characters".to_string())
    })?;

    let system = OsSystem::new(&cwd_sys);
    let metadata = ProjectMetadata::new(Name::new("spython"), cwd_sys);

    ProjectDatabase::new(metadata, system).map_err(|e| CheckError::DatabaseBuild(e.to_string()))
}

/// Run spython's annotation checker on all files in the database.
///
/// Returns a vector of diagnostics for any missing annotations found.
fn annotation_check(db: &ProjectDatabase) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Get files from the database's project
    for file in &db.project().files(db) {
        let file_diagnostics = checker::check_file_annotations(db, file);
        diagnostics.extend(file_diagnostics);
    }

    diagnostics
}

/// Display diagnostics to the user.
///
/// Shows formatted diagnostics with source code snippets and a summary count.
/// The `diagnostic_type` parameter should be singular (e.g., "error", "diagnostic").
fn display_diagnostics(db: &ProjectDatabase, diagnostics: &[Diagnostic], diagnostic_type: &str) {
    let use_color = std::io::stderr().is_terminal();
    let config = DisplayDiagnosticConfig::new("spython").color(use_color);
    eprint!("{}", DisplayDiagnostics::new(db, &config, diagnostics));

    let n = diagnostics.len();
    let plural_suffix = if n == 1 { "" } else { "s" };
    eprintln!("Found {n} {diagnostic_type}{plural_suffix}");
}
