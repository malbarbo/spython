pub mod output;

pub mod checker;
pub mod completion;
pub mod doctests;
pub mod lints;
pub mod panic;
pub mod parser;
pub mod repl;
pub mod spython_lib;
pub mod wasm_ffi;

pub use checker::Level;
pub use repl::{
    REPL_ERROR, REPL_OK, REPL_QUIT, ReplOutcome, ReplSession, ReplState, repl_new, repl_run,
};

use std::collections::HashSet;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use ruff_db::diagnostic::{Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{
    InMemorySystem, MemoryFileSystem, SystemPath, SystemPathBuf, WritableSystem,
};
use ruff_python_ast::name::Name;
use ruff_python_ast::{ExprRef, Stmt};
use ruff_python_formatter::{DocstringCode, PyFormatOptions, QuoteStyle, format_module_source};
use ruff_ranged_value::RangedValue;
use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
pub use ty_project::ProjectDatabase;
use ty_project::metadata::Options;
use ty_project::metadata::options::{EnvironmentOptions, Rules};
use ty_project::metadata::value::RelativePathBuf;
use ty_project::{Db, ProjectMetadata};
use ty_python_semantic::Db as _;
use ty_python_semantic::lint::Level as TyLevel;
use ty_python_semantic::{HasType, SemanticModel};

const PROJECT_ROOT: &str = "/";
const USER_FILE: &str = "/user.py";
/// Where the modules a session started from go. A root of their own, on the
/// search path rather than beside the source, so no file the student wrote
/// can land on [`USER_FILE`] and quietly replace the code being checked.
const MODULE_ROOT: &str = "/modules";

/// Package version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Libraries bundled in this build (set by build.rs).
pub const LIBS_VERSION: &str = env!("LIBS_VERSION");
/// Build date in YYYY-MM-DD format (set by build.rs).
pub const BUILD_DATE: &str = env!("BUILD_DATE");
/// Short git commit hash (set by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");
/// Full version string: "0.1.0 (rustpython ..., 2026-04-01, abc1234)".
pub const LONG_VERSION: &str = env!("LONG_VERSION");

/// The doctest runner Python source. Shared between the CLI `check` command
/// and the WASM REPL so the string is embedded only once per binary.
pub const DOCTEST_RUNNER: &str = include_str!("doctest_runner.py");

/// Errors returned when type checking finds problems in source code.
pub struct TypeErrors {
    pub db: ProjectDatabase,
    pub diagnostics: Vec<Diagnostic>,
}

/// Create a `ProjectMetadata` with spython's lint rule overrides applied.
///
/// Promotes a few ty rules from `Ignore`/`Warn` to `Error` for the teaching
/// context: `possibly-unresolved-reference` (variable assigned only in some
/// branches) and `division-by-zero` (literal `/ 0`, `% 0`, etc.).
///
/// `extra_paths` is added to ty's module search path — used to surface the
/// embedded spython library so `from spython import ...` resolves.
pub fn make_project_metadata(
    cwd: SystemPathBuf,
    extra_paths: Vec<SystemPathBuf>,
) -> ProjectMetadata {
    let mut metadata = ProjectMetadata::new(Name::new("spython"), cwd);
    let rules = Rules::from_iter([
        (
            RangedValue::cli("possibly-unresolved-reference".to_string()),
            RangedValue::cli(TyLevel::Error),
        ),
        (
            RangedValue::cli("division-by-zero".to_string()),
            RangedValue::cli(TyLevel::Error),
        ),
    ]);
    let environment = (!extra_paths.is_empty()).then(|| EnvironmentOptions {
        extra_paths: Some(
            extra_paths
                .into_iter()
                .map(|p| RelativePathBuf::cli(p.as_path()))
                .collect(),
        ),
        ..EnvironmentOptions::default()
    });
    metadata.apply_override_options(Options {
        rules: Some(rules),
        environment,
        ..Options::default()
    });
    metadata
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

/// A project over an in-memory filesystem holding `source` as `/user.py`, the
/// spython library, and `modules`.
///
/// `modules` are the first-party files the source imports, each a path
/// relative to the directory the source came from and its text. Nothing on
/// the real filesystem is reachable from here, so `import helpers` only
/// resolves if `helpers.py` is among them. They go under a search path of
/// their own, next to the library's: reachable by name, out of reach of the
/// source's own path, and not included, so the checker reads them to resolve
/// names and reports nothing about them.
fn in_memory_project(source: &str, modules: &[(String, String)]) -> (ProjectDatabase, File) {
    let cwd = SystemPathBuf::from(PROJECT_ROOT);
    let system = InMemorySystem::from_memory_fs(MemoryFileSystem::with_current_directory(&cwd));
    system
        .write_file(SystemPath::new(USER_FILE), source)
        .expect("writing to in-memory filesystem should never fail");
    for (path, module_source) in modules {
        // `write_file_all`, not `write_file`: a module under a package needs
        // its directory to exist first.
        system
            .fs()
            .write_file_all(
                SystemPathBuf::from(format!("{MODULE_ROOT}/{path}")),
                module_source,
            )
            .expect("writing to in-memory filesystem should never fail");
    }
    let lib_root = spython_lib::write_into(&system, SystemPath::new(PROJECT_ROOT));

    let mut search_paths = vec![lib_root];
    if !modules.is_empty() {
        search_paths.push(SystemPathBuf::from(MODULE_ROOT));
    }
    let metadata = make_project_metadata(cwd, search_paths);
    let mut db = ProjectDatabase::fallible(metadata, system)
        .expect("building ProjectDatabase with in-memory system should never fail");

    let file_path = SystemPathBuf::from(USER_FILE);
    let user_file = system_path_to_file(&db, &file_path)
        .expect("user.py should be resolvable after writing it to the in-memory filesystem");

    db.project().set_included_paths(&mut db, vec![file_path]);
    (db, user_file)
}

/// Type-check `source` entirely in memory (no OS filesystem access).
///
/// Used by the WASM shim, which has one file and nothing to import.
pub fn type_check_source(
    source: &str,
    level: Level,
    in_repl: bool,
) -> Result<Option<Box<TypeErrors>>, Box<TypeErrors>> {
    type_check_in_memory(source, level, in_repl, &[])
}

/// Type-check `source` against `modules` -- the first-party files it imports.
fn type_check_in_memory(
    source: &str,
    level: Level,
    in_repl: bool,
    modules: &[(String, String)],
) -> Result<Option<Box<TypeErrors>>, Box<TypeErrors>> {
    let (db, user_file) = in_memory_project(source, modules);

    let mut diagnostics = annotation_check(&db, level, in_repl);
    diagnostics.extend(db.check());
    // Doctests are code too — validate prompt format and type-check snippets.
    // Skip in REPL context, where incremental input doesn't have docstrings.
    if !in_repl {
        diagnostics.extend(check_file_doctests(&db, user_file, level));
    }

    let has_errors = diagnostics
        .iter()
        .any(|d| d.severity() == ruff_db::diagnostic::Severity::Error);
    if has_errors {
        Err(Box::new(TypeErrors { db, diagnostics }))
    } else if !diagnostics.is_empty() {
        Ok(Some(Box::new(TypeErrors { db, diagnostics })))
    } else {
        Ok(None)
    }
}

/// Run the annotation checker on all files registered in `db`.
pub fn annotation_check(db: &ProjectDatabase, level: Level, in_repl: bool) -> Vec<Diagnostic> {
    // `Project::files` is backed by an `FxHashSet`, whose iteration order is
    // not deterministic across platforms. Sort by path so the diagnostic
    // stream — and the snapshot tests that consume it — are stable.
    let mut files: Vec<_> = db.project().files(db).into_iter().collect();
    files.sort_by_key(|f| f.path(db).as_str().to_owned());
    let mut diagnostics = Vec::new();
    for file in files {
        // Skip the embedded spython library — only check student files.
        let is_spython_library = file
            .path(db)
            .as_system_path()
            .is_some_and(spython_lib::is_lib_path);
        if !is_spython_library {
            diagnostics.extend(checker::check_file_annotations(db, file, level, in_repl));
        }
    }
    diagnostics
}

/// The files among `files` that `main_file` can name, keyed by their path
/// relative to the directory it sits in -- the form [`in_memory_project`]
/// takes its modules in.
///
/// An in-memory check cannot reach the real directory, so what the source
/// imports has to travel as text. `main_file` itself is left out, and so is
/// a file outside its directory: there is no relative name that would reach
/// it, so writing it would not help the resolver either. Sorted, because
/// `files` often comes from a `HashSet`.
pub fn modules_beside(
    db: &ProjectDatabase,
    main_file: File,
    files: impl IntoIterator<Item = File>,
) -> Vec<(String, String)> {
    let Some(root) = main_file.path(db).as_system_path().and_then(|p| p.parent()) else {
        return Vec::new();
    };
    let mut modules: Vec<(String, String)> = files
        .into_iter()
        .filter(|f| *f != main_file)
        .filter_map(|f| {
            let path = f.path(db).as_system_path()?;
            let relative = path.strip_prefix(root).ok()?;
            let source = ruff_db::source::source_text(db, f);
            Some((relative.as_str().to_owned(), source.to_string()))
        })
        .collect();
    modules.sort();
    modules
}

/// Type-check and format-check doctests inside a file's docstrings.
///
/// Returns diagnostics for:
/// * malformed prompts (`>>>x` / `...x`) — always reported;
/// * annotation rules / level restrictions / ty type errors found inside the
///   doctest snippets, remapped to the original docstring locations.
///
/// Doctest diagnostics complement the ordinary file-level check; callers
/// should append this to their existing diagnostic list.
pub fn check_file_doctests(db: &ProjectDatabase, file: File, level: Level) -> Vec<Diagnostic> {
    let parsed = parsed_module(db, db.program_file(file).python_file(db));
    let module = parsed.load(db);
    let source = ruff_db::source::source_text(db, file);
    let source_str = source.as_str();

    let extraction = doctests::extract_doctests(module.syntax(), source_str);

    let mut diagnostics = doctests::malformed_prompt_diagnostics(file, &extraction.malformed);

    if extraction.snippets.is_empty() {
        return diagnostics;
    }

    let (syn_source, syn_map) = doctests::build_synthetic_source(source_str, &extraction.snippets);

    // A doctest can name what the file around it imports, so the check gets
    // the file's first-party imports as the modules to resolve against --
    // the same in-memory project the REPL checks its inputs in.
    let mut transitive: HashSet<File> = HashSet::new();
    collect_import_files(db, file, &mut transitive);
    let modules = modules_beside(db, file, transitive);

    let (scratch, syn_file) = in_memory_project(&syn_source, &modules);

    let mut raw = annotation_check(&scratch, level, false);
    raw.extend(scratch.check());

    let remapped = doctests::remap_diagnostics(
        raw,
        syn_file,
        file,
        &extraction.snippets,
        &syn_map,
        &syn_source,
    );
    diagnostics.extend(remapped);
    diagnostics
}

/// Recursively collect first-party files transitively imported by `file`.
pub fn collect_import_files(db: &ProjectDatabase, file: File, seen: &mut HashSet<File>) {
    let parsed = parsed_module(db, db.program_file(file).python_file(db));
    let module = parsed.load(db);

    for stmt in module.suite() {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    // Skip library modules (spython) — not student code.
                    if alias.name.id.starts_with("spython") {
                        continue;
                    }
                    if let Some(name) = ModuleName::new(alias.name.id.as_str()) {
                        visit_module(db, file, &name, seen);
                    }
                }
            }
            Stmt::ImportFrom(import_from) => {
                let module_str = import_from.module.as_deref();
                // Skip library modules (spython) — not student code.
                if module_str.is_some_and(|m| m.starts_with("spython")) {
                    continue;
                }
                if let Ok(name) = ModuleName::from_identifier_parts(
                    db,
                    importing_file(db, file),
                    module_str,
                    import_from.level,
                ) {
                    visit_module(db, file, &name, seen);
                }
            }
            _ => {}
        }
    }
}

/// Build the module-resolution key ty now requires for the importing file.
fn importing_file<'db>(db: &'db ProjectDatabase, file: File) -> ImportingFile<'db> {
    ImportingFile::ResolverFile(db.program_file(file).resolver_file(db))
}

fn visit_module(
    db: &ProjectDatabase,
    importing: File,
    name: &ModuleName,
    seen: &mut HashSet<File>,
) {
    if let Some(module) = resolve_module(db, importing_file(db, importing), name)
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
    let options = PyFormatOptions::default()
        .with_docstring_code(DocstringCode::Enabled)
        .with_quote_style(QuoteStyle::Single);
    format_module_source(source, options)
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
        register_ffi_module(vm);
        vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
        vm.run_string(scope, &source, &filename).map(drop)
    });
    code == 0
}

/// Print type errors to stderr. Pass `use_color = true` in WASM (ansi.ts renders
/// the ANSI codes), or `stderr().is_terminal()` from the CLI binary.
pub fn print_type_errors(db: &ProjectDatabase, diagnostics: &[Diagnostic], use_color: bool) {
    use ruff_db::diagnostic::Severity;
    use std::fmt::Write;
    let config = DisplayDiagnosticConfig::new("spython").color(use_color);
    let n = diagnostics
        .iter()
        .filter(|d| d.severity() == Severity::Error)
        .count();
    let mut buf = format!("{}", DisplayDiagnostics::new(db, &config, diagnostics));
    if n > 0 {
        let s = if n == 1 { "" } else { "s" };
        let _ = writeln!(buf, "Found {n} error{s}.");
    }
    eprint!("{buf}");
}

/// Type-check a REPL input in the context of previously accumulated source.
///
/// Checks `accumulated + "\n" + new_input` for cross-reference correctness,
/// resolving imports against `modules` -- the first-party files the session
/// started from, which are not on any filesystem the checker can see.
/// Diagnostics about the input are printed with their line numbers relative
/// to it; diagnostics about the accumulated source are dropped.
///
/// Returns `true` if the input passed type checking, `false` if errors were found.
fn type_check_repl_input(
    accumulated: &str,
    new_input: &str,
    level: Level,
    use_color: bool,
    modules: &[(String, String)],
) -> bool {
    let accumulated = accumulated.trim_end_matches('\n');
    let new_input = new_input.trim_end_matches('\n');
    let combined = if accumulated.is_empty() {
        new_input.to_owned()
    } else {
        format!("{accumulated}\n{new_input}")
    };

    let te = match type_check_in_memory(&combined, level, true, modules) {
        Ok(_) => return true, // warnings don't block execution
        Err(te) => te,
    };

    // Where the input starts in `combined`. What lands before it is about the
    // accumulated source, which no prompt can reach: printing it would blame
    // what the user just typed for code they cannot fix from here.
    let input_offset = combined.len() - new_input.len();
    let in_input: Vec<Placed> = te
        .diagnostics
        .iter()
        .filter_map(|diag| match diagnostic_range(diag) {
            // No range means the diagnostic is about the check as a whole,
            // not about a line of history, so it stays.
            None => Some(Placed { diag, at: None }),
            Some((start, end)) if start >= input_offset => {
                let (line, col) = line_and_column(new_input, start - input_offset);
                Some(Placed {
                    diag,
                    at: Some((line, col, end - start)),
                })
            }
            Some(_) => None,
        })
        .collect();
    if !in_input
        .iter()
        .any(|p| p.diag.severity() == ruff_db::diagnostic::Severity::Error)
    {
        return true;
    }

    let new_input_lines: Vec<&str> = new_input.lines().collect();
    print_repl_diagnostics(&in_input, &new_input_lines, use_color);

    false
}

/// Infer the static type of a Python expression in the REPL context.
///
/// Builds a source with `accumulated + "\n" + expr`, type-checks it against
/// `modules` the way [`type_check_repl_input`] does, and returns the display
/// string of the inferred type for the expression.
fn infer_expression_type(
    accumulated: &str,
    expr: &str,
    modules: &[(String, String)],
) -> Option<String> {
    // Wrap the expression as the last statement so we can find it in the AST.
    let source = if accumulated.is_empty() {
        expr.to_owned()
    } else {
        format!("{}\n{}", accumulated.trim_end_matches('\n'), expr)
    };

    let (db, file) = in_memory_project(&source, modules);

    // Parse and find the last expression statement.
    let program_file = db.program_file(file);
    let parsed = parsed_module(&db, program_file.python_file(&db));
    let module = parsed.load(&db);
    let stmts = module.suite();
    let last_stmt = stmts.last()?;

    // The last statement should be an expression statement.
    let expr_node = match last_stmt {
        Stmt::Expr(expr_stmt) => &*expr_stmt.value,
        _ => return None,
    };

    let model = SemanticModel::new(&db, program_file);
    let ty = ExprRef::from(expr_node).inferred_type(&model)?;
    let type_str = ty.display(&db, &model.program_environment()).to_string();
    Some(simplify_type_display(&type_str))
}

/// Simplify ty's type display for student-friendly output.
///
/// Converts `Literal[10]` → `int`, `Literal["hello"]` → `str`, etc.
fn simplify_type_display(s: &str) -> String {
    if let Some(inner) = s.strip_prefix("Literal[").and_then(|s| s.strip_suffix(']')) {
        // Determine the base type from the literal value.
        if inner == "True" || inner == "False" {
            return "bool".to_owned();
        }
        if inner.starts_with('"') || inner.starts_with('\'') {
            return "str".to_owned();
        }
        if inner.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
            return "int".to_owned();
        }
        // Unknown literal — return as-is.
        return s.to_owned();
    }
    s.to_owned()
}

/// The byte range a diagnostic points at.
///
/// `None` for one that carries no range -- ty reports an unreadable file that
/// way. It is nowhere in particular, which is not the same as being at the
/// start, and the caller has to keep the two apart.
fn diagnostic_range(diag: &Diagnostic) -> Option<(usize, usize)> {
    diag.primary_span()
        .and_then(|span| span.range())
        .map(|range| (usize::from(range.start()), usize::from(range.end())))
}

/// Where `offset` falls in `source`: the 0-based line and 0-based column.
fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let line = source[..offset].matches('\n').count();
    let line_start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    (line, offset - line_start)
}

/// A diagnostic and where it falls inside the REPL input: 0-based line,
/// 0-based column, and how many bytes it underlines. `None` for one that
/// carries no range, which is shown against the first line for want of a
/// better place.
struct Placed<'a> {
    diag: &'a Diagnostic,
    at: Option<(usize, usize, usize)>,
}

/// Print diagnostics already placed inside the REPL input.
fn print_repl_diagnostics(diagnostics: &[Placed], new_input_lines: &[&str], use_color: bool) {
    use ruff_db::diagnostic::Severity;
    let mut error_count = 0;

    for Placed { diag, at } in diagnostics {
        let is_error = diag.severity() == Severity::Error;
        if is_error {
            error_count += 1;
        }

        let (adjusted_line, col, span_len) = at.unwrap_or((0, 0, 1));
        let display_line = adjusted_line + 1; // 1-indexed

        let severity = match (is_error, use_color) {
            (true, true) => "\x1b[1;91merror",
            (true, false) => "error",
            (false, true) => "\x1b[1;93mwarning",
            (false, false) => "warning",
        };

        let id = diag.id();
        let msg = diag.headline_message();
        let reset = if use_color { "\x1b[0m" } else { "" };
        let bold = if use_color { "\x1b[1m" } else { "" };

        eprintln!("{severity}[{id}]{reset}: {bold}{msg}{reset}");
        eprintln!(" --> user.py:{display_line}:{}", col + 1);

        // Show the source line if available.
        if let Some(src_line) = new_input_lines.get(adjusted_line) {
            eprintln!("  |");
            eprintln!("{display_line} | {src_line}");
            let padding = " ".repeat(col);
            let underline_len = span_len.max(1).min(src_line.len().saturating_sub(col));
            let underline = "^".repeat(underline_len);
            if use_color {
                eprintln!("  | {padding}\x1b[1;91m{underline}\x1b[0m");
            } else {
                eprintln!("  | {padding}{underline}");
            }
            eprintln!("  |");
        }
    }

    if error_count > 0 {
        let s = if error_count == 1 { "" } else { "s" };
        eprintln!("Found {error_count} error{s}.");
    }
}

/// Set by the `__test_mark_atexit_ran` function that `register_ffi_module`
/// adds under test, so [`crate::repl`] can see that dropping a session ran
/// Python's atexit handlers.
#[cfg(test)]
static REPL_ATEXIT_RAN: AtomicBool = AtomicBool::new(false);

/// Register the `_spython_ffi` module with native `show_svg` and `get_key_event` functions.
fn register_ffi_module(vm: &vm::VirtualMachine) {
    use rustpython::vm::PyObjectRef;

    let show_svg_fn = vm.new_function(
        "show_svg",
        |svg: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            wasm_ffi::show_svg(&svg);
            Ok(vm.ctx.none())
        },
    );

    let get_key_event_fn =
        vm.new_function("get_key_event", |vm: &vm::VirtualMachine| -> PyObjectRef {
            match wasm_ffi::poll_key_event() {
                None => vm.ctx.none(),
                Some((event_type, key, mods)) => {
                    let elements = vec![
                        vm.ctx.new_int(event_type).into(),
                        vm.ctx.new_str(key).into(),
                        vm.ctx.new_bool(mods[0]).into(), // alt
                        vm.ctx.new_bool(mods[1]).into(), // ctrl
                        vm.ctx.new_bool(mods[2]).into(), // shift
                        vm.ctx.new_bool(mods[3]).into(), // meta
                        vm.ctx.new_bool(mods[4]).into(), // repeat
                    ];
                    vm.ctx.new_tuple(elements).into()
                }
            }
        });

    let text_metrics_fn = vm.new_function(
        "text_metrics",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            let (width, height, x_offset, y_offset) = wasm_ffi::measure_text(&text, &font_css);
            let elements = vec![
                vm.ctx.new_float(width).into(),
                vm.ctx.new_float(height).into(),
                vm.ctx.new_float(x_offset).into(),
                vm.ctx.new_float(y_offset).into(),
            ];
            vm.ctx.new_tuple(elements).into()
        },
    );

    let load_bitmap_fn = vm.new_function(
        "load_bitmap",
        |_path: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            Err(vm.new_runtime_error("load_bitmap is not available in native mode".to_owned()))
        },
    );
    #[cfg(test)]
    let test_mark_atexit_ran_fn = vm.new_function(
        "__test_mark_atexit_ran",
        |vm: &vm::VirtualMachine| -> vm::PyObjectRef {
            REPL_ATEXIT_RAN.store(true, Ordering::SeqCst);
            vm.ctx.none()
        },
    );

    // A fresh dict takes any name, so setting one of these cannot fail.
    let dict = vm.ctx.new_dict();
    let set = |name: &str, value: vm::PyObjectRef| {
        dict.set_item(name, value, vm)
            .expect("a fresh dict takes a name");
    };
    set("show_svg", show_svg_fn.into());
    set("get_key_event", get_key_event_fn.into());
    set("text_metrics", text_metrics_fn.into());
    set("load_bitmap", load_bitmap_fn.into());
    #[cfg(test)]
    set("__test_mark_atexit_ran", test_mark_atexit_ran_fn.into());
    let module = vm.new_module("_spython_ffi", dict, None);

    let sys_modules = vm
        .sys_module
        .get_attr("modules", vm)
        .expect("sys.modules should exist");
    sys_modules
        .set_item("_spython_ffi", module.into(), vm)
        .expect("should be able to add to sys.modules");
}

/// Replace sys.stdout and sys.stderr with writers that route output
/// to the thread-local capture buffers (see `output.rs`).
#[cfg(feature = "capture")]
fn install_capture_writers(vm: &vm::VirtualMachine) {
    fn make_writer(vm: &vm::VirtualMachine, name: &str, write_fn: fn(&str)) -> vm::PyObjectRef {
        let write = vm.new_function("write", move |s: String| -> usize {
            let len = s.len();
            write_fn(&s);
            len
        });
        let flush = vm.new_function("flush", || {});
        let dict = vm.ctx.new_dict();
        dict.set_item("write", write.into(), vm).unwrap();
        dict.set_item("flush", flush.into(), vm).unwrap();
        vm.new_module(name, dict, None).into()
    }

    let stdout = make_writer(vm, "_capture_stdout", output::write_stdout);
    let stderr = make_writer(vm, "_capture_stderr", output::write_stderr);
    vm.sys_module
        .set_attr("stdout", stdout, vm)
        .expect("set sys.stdout");
    vm.sys_module
        .set_attr("stderr", stderr, vm)
        .expect("set sys.stderr");
}

#[cfg(test)]
mod repl_check_tests {
    use super::*;

    fn helpers() -> Vec<(String, String)> {
        vec![(
            "helpers.py".to_owned(),
            "def helper(x: int) -> int:\n    return x * 2\n".to_owned(),
        )]
    }

    #[test]
    fn input_resolves_a_module_the_session_started_from() {
        assert!(type_check_repl_input(
            "import helpers",
            "helpers.helper(1)",
            Level::Full,
            false,
            &helpers(),
        ));
    }

    #[test]
    fn input_resolves_a_module_under_a_package() {
        let modules = vec![(
            "pkg/util.py".to_owned(),
            "def twice(x: int) -> int:\n    return x * 2\n".to_owned(),
        )];
        assert!(type_check_repl_input(
            "from pkg.util import twice",
            "twice(1)",
            Level::Full,
            false,
            &modules,
        ));
    }

    #[test]
    fn a_module_named_like_the_checked_file_does_not_replace_it() {
        let modules = vec![(
            "user.py".to_owned(),
            "def helper(x: int) -> int:\n    return x * 2\n".to_owned(),
        )];
        // The input is still what gets checked...
        assert!(!type_check_repl_input(
            "import user",
            "undefined_name",
            Level::Full,
            false,
            &modules,
        ));
        // ...and the module is still what `user` names.
        assert!(type_check_repl_input(
            "import user",
            "user.helper(1)",
            Level::Full,
            false,
            &modules,
        ));
    }

    #[test]
    fn a_module_the_session_never_saw_is_still_an_error() {
        assert!(!type_check_repl_input(
            "",
            "import helpers",
            Level::Full,
            false,
            &[],
        ));
    }

    #[test]
    fn input_is_not_blamed_for_an_error_in_what_ran_before() {
        // The error is on line 1, which belongs to the accumulated source; the
        // input itself is fine, and is what the user can still fix.
        assert!(type_check_repl_input(
            "undefined_name",
            "1 + 1",
            Level::Full,
            false,
            &[],
        ));
    }

    #[test]
    fn an_error_in_the_input_is_still_reported() {
        assert!(!type_check_repl_input(
            "x: int = 1",
            "undefined_name",
            Level::Full,
            false,
            &[],
        ));
    }

    #[test]
    fn a_type_is_inferred_through_a_module_the_session_started_from() {
        assert_eq!(
            infer_expression_type("import helpers", "helpers.helper(1)", &helpers()),
            Some("int".to_owned())
        );
    }
}
