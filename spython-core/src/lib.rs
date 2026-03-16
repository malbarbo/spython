use std::collections::HashSet;

use ruff_db::diagnostic::{Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{InMemorySystem, SystemPath, SystemPathBuf, WritableSystem};
use ruff_python_ast::Stmt;
use ruff_python_ast::name::Name;
use ruff_python_formatter::{PyFormatOptions, format_module_source};
use rustpython::vm::AsObject;
use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};
use ty_module_resolver::{ModuleName, resolve_module};
pub use ty_project::ProjectDatabase;
use ty_project::{Db, ProjectMetadata};

pub mod checker;
pub mod lints;

const PROJECT_ROOT: &str = "/";
const USER_FILE: &str = "/user.py";

/// Errors returned when type checking finds problems in source code.
pub struct TypeErrors {
    pub db: ProjectDatabase,
    pub diagnostics: Vec<Diagnostic>,
}

/// Create a RustPython interpreter with the standard library loaded.
pub fn new_interpreter() -> vm::Interpreter {
    let mut settings = vm::Settings::default();
    settings.write_bytecode = false;
    InterpreterBuilder::new()
        .settings(settings)
        .init_stdlib()
        .interpreter()
}

/// Type-check `source` entirely in memory (no OS filesystem access).
///
/// Used by the WASM shim. The source is written to an in-memory filesystem
/// under the path `/user.py`, then annotation-checked and ty-checked.
pub fn type_check_source(source: &str) -> Result<(), Box<TypeErrors>> {
    let cwd = SystemPathBuf::from(PROJECT_ROOT);
    let system = InMemorySystem::new(cwd.clone());
    system
        .write_file(SystemPath::new(USER_FILE), source)
        .expect("writing to in-memory filesystem should never fail");

    let metadata = ProjectMetadata::new(Name::new("spython"), cwd);
    let mut db = ProjectDatabase::new(metadata, system)
        .expect("building ProjectDatabase with in-memory system should never fail");

    let file_path = SystemPathBuf::from(USER_FILE);
    let _file = system_path_to_file(&db, &file_path)
        .expect("user.py should be resolvable after writing it to the in-memory filesystem");

    db.project().set_included_paths(&mut db, vec![file_path]);

    let mut diagnostics = annotation_check(&db);
    diagnostics.extend(db.check());

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(Box::new(TypeErrors { db, diagnostics }))
    }
}

/// Run the annotation checker on all files registered in `db`.
pub fn annotation_check(db: &ProjectDatabase) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in &db.project().files(db) {
        diagnostics.extend(checker::check_file_annotations(db, file));
    }
    diagnostics
}

/// Recursively collect first-party files transitively imported by `file`.
pub fn collect_import_files(db: &ProjectDatabase, file: File, seen: &mut HashSet<File>) {
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

fn visit_module(
    db: &ProjectDatabase,
    importing_file: File,
    name: &ModuleName,
    seen: &mut HashSet<File>,
) {
    if let Some(module) = resolve_module(db, importing_file, name)
        && module.search_path(db).is_some_and(|sp| sp.is_first_party())
        && let Some(mod_file) = module.file(db)
        && seen.insert(mod_file)
    {
        collect_import_files(db, mod_file, seen);
    }
}

/// Format Python source with ruff's formatter.
/// Returns `Err(message)` if the source cannot be parsed.
pub fn format_source(source: &str) -> Result<String, String> {
    format_module_source(source, PyFormatOptions::default())
        .map(|f| f.into_code())
        .map_err(|e| e.to_string())
}

/// Execute Python source in a fresh interpreter. Returns `true` on success.
pub fn execute_source(source: &str, filename: &str, parent_dir: &str) -> bool {
    let interp = new_interpreter();
    let source = source.to_owned();
    let filename = filename.to_owned();
    let parent_dir = parent_dir.to_owned();
    let code = interp.run(move |vm| {
        let scope = vm.new_scope_with_main()?;
        vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
        vm.run_string(scope, &source, filename).map(drop)
    });
    code == 0
}

/// Print type errors to stderr. Pass `use_color = true` in WASM (ansi.ts renders
/// the ANSI codes), or `stderr().is_terminal()` from the CLI binary.
pub fn print_type_errors(db: &ProjectDatabase, diagnostics: &[Diagnostic], use_color: bool) {
    let config = DisplayDiagnosticConfig::new("spython").color(use_color);
    eprint!("{}", DisplayDiagnostics::new(db, &config, diagnostics));
    let n = diagnostics.len();
    let s = if n == 1 { "" } else { "s" };
    eprintln!("Found {n} error{s}.");
}

// --- REPL state ---

/// Persistent Python interpreter state for the web REPL.
pub struct ReplState {
    // `scope` is dropped before `interp` because Rust drops fields top-to-bottom,
    // and the scope must be freed while the VM is still alive.
    scope: Option<vm::scope::Scope>,
    interp: vm::Interpreter,
}

impl Drop for ReplState {
    fn drop(&mut self) {
        // Drop the scope inside the VM context so Python __del__ methods can run.
        let scope = self.scope.take();
        self.interp.enter(|_| drop(scope));
    }
}

/// Create a new REPL session, optionally preloading `source`.
///
/// If `source` is non-empty it is executed into the scope so its definitions
/// are available to subsequent `repl_run` calls. Type errors are printed to
/// stderr and execution continues (the REPL is still created).
pub fn repl_new(source: &str) -> Box<ReplState> {
    let mut settings = vm::Settings::default();
    settings.write_bytecode = false;
    let interp = InterpreterBuilder::new()
        .settings(settings)
        .init_stdlib()
        .interpreter();
    let scope = interp.enter(|vm| {
        let scope = vm
            .new_scope_with_main()
            .expect("creating the main scope should not fail");
        if !source.trim().is_empty() {
            if let Err(exc) = vm.run_string(scope.clone(), source, "user.py".to_owned()) {
                vm.print_exception(exc);
            } else {
                // Run doctests using the custom runner (avoids stdlib doctest
                // which needs _io.FileIO, unavailable on WASM).
                let doctest_code = concat!(
                    include_str!("doctest_runner.py"),
                    "\nimport __main__; run_doctests(__main__); del __main__\n"
                );
                if let Err(exc) = vm.run_string(scope.clone(), doctest_code, "<doctest>".to_owned())
                {
                    vm.print_exception(exc);
                }
            }
        }
        scope
    });
    Box::new(ReplState {
        scope: Some(scope),
        interp,
    })
}

/// Execute one REPL expression/statement in the session's scope.
///
/// Returns `true` if the user called `exit()` / `quit()` (SystemExit raised),
/// meaning the caller should restart or close the session.
pub fn repl_run(state: &mut ReplState, code: &str) -> bool {
    let scope = state.scope.as_ref().unwrap().clone();
    let code = code.to_owned();
    state.interp.enter(move |vm| {
        match vm
            .compile(&code, vm::compiler::Mode::Single, "<stdin>".to_owned())
            .map_err(|err| vm.new_syntax_error(&err, Some(&code)))
            .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
        {
            Ok(_) => false,
            Err(exc) => {
                if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                    true
                } else {
                    vm.print_exception(exc);
                    false
                }
            }
        }
    })
}
