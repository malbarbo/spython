use clap::Parser;
use ruff_db::diagnostic::{Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{OsSystem, SystemPathBuf};
use ruff_python_ast::Stmt;
use ruff_python_ast::name::Name;
use rustpython::{InterpreterBuilder, InterpreterBuilderExt, run_shell};
use rustpython::vm::Settings;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ty_module_resolver::{ModuleName, resolve_module};
use ty_project::{Db, ProjectDatabase, ProjectMetadata};

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
    /// Failed to read the script file
    ScriptRead(std::io::Error),
    /// Script execution failed
    ScriptExecution,
}

/// spython: A Python interpreter with integrated type checking for students
#[derive(Parser)]
#[clap(name = "spython", version)]
struct Cli {
    /// Python script to run (or REPL if omitted)
    file: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.file {
        Some(ref path) => match run_checked(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                display_error(e);
                ExitCode::FAILURE
            }
        },
        None => start_repl(),
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
        CheckError::ScriptRead(e) => {
            eprintln!("spython: cannot read script: {e}");
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

/// Start a Python REPL without type checking.
fn start_repl() -> ExitCode {
    let interp = new_interpreter();
    let code = interp.run(|vm| {
        let scope = vm.new_scope_with_main()?;
        vm.sys_module.set_attr("ps1", vm.ctx.new_str("> "), vm)?;
        vm.sys_module.set_attr("ps2", vm.ctx.new_str("  "), vm)?;
        run_shell(vm, scope)
    });
    if code == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Run a Python script with type checking enabled.
///
/// This function:
/// 1. Builds a ty database rooted at the current directory
/// 2. Uses ty's module resolver to collect the transitive import closure
/// 3. Runs annotation checker first
/// 4. If annotations are OK, runs ty's type checker
/// 5. If no errors are found, executes the script with RustPython
fn run_checked(file: &Path) -> Result<(), CheckError> {
    // Validate the script path before building the database.
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

    // ── Build the database and set included paths ──────────────────────
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

    // Collect all transitively imported first-party files using ty's resolver.
    let mut local_files: HashSet<File> = HashSet::new();
    local_files.insert(main_file);
    collect_local_imports(&db, main_file, &mut local_files);

    let sys_files: Vec<SystemPathBuf> = local_files
        .iter()
        .filter_map(|f| f.path(&db).as_system_path().map(|p| p.to_path_buf()))
        .collect();
    db.project().set_included_paths(&mut db, sys_files);

    // ── Check annotations first (spython's custom lints) ───────────────
    let annotation_diagnostics = annotation_check(&db);
    if !annotation_diagnostics.is_empty() {
        return Err(CheckError::AnnotationErrors(db, annotation_diagnostics));
    }

    // ── Type check using ty (only if annotations are complete) ─────────
    let type_diagnostics = db.check();
    if !type_diagnostics.is_empty() {
        return Err(CheckError::TypeErrors(db, type_diagnostics));
    }

    // ── Run the script ─────────────────────────────────────────────────
    let source = std::fs::read_to_string(file).map_err(CheckError::ScriptRead)?;

    let file_str = file.to_string_lossy().into_owned();
    let parent_dir = file
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_owned();

    let interp = new_interpreter();
    let code = interp.run(move |vm| {
        let scope = vm.new_scope_with_main()?;
        // Make imports relative to the script's directory work.
        vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
        vm.run_string(scope, &source, file_str).map(drop)
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
