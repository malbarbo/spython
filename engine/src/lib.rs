pub mod output;

pub mod checker;
pub mod completion;
pub mod doctests;
pub mod lints;
pub mod panic;
pub mod wasm_ffi;

use std::collections::HashSet;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use ruff_db::diagnostic::{Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{InMemorySystem, SystemPath, SystemPathBuf, WritableSystem};
use ruff_python_ast::name::Name;
use ruff_python_ast::{ExprRef, Stmt};
use ruff_python_formatter::{PyFormatOptions, format_module_source};
use rustpython::vm::AsObject;
use rustpython::{InterpreterBuilder, InterpreterBuilderExt, vm};
use ty_module_resolver::{ModuleName, resolve_module};
pub use ty_project::ProjectDatabase;
use ty_project::{Db, ProjectMetadata};
use ty_python_semantic::{HasType, SemanticModel};

const PROJECT_ROOT: &str = "/";
const USER_FILE: &str = "/user.py";

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
pub fn type_check_source(
    source: &str,
    level: Level,
    in_repl: bool,
) -> Result<Option<Box<TypeErrors>>, Box<TypeErrors>> {
    let cwd = SystemPathBuf::from(PROJECT_ROOT);
    let system = InMemorySystem::new(cwd.clone());
    system
        .write_file(SystemPath::new(USER_FILE), source)
        .expect("writing to in-memory filesystem should never fail");

    let metadata = ProjectMetadata::new(Name::new("spython"), cwd);
    let mut db = ProjectDatabase::new(metadata, system)
        .expect("building ProjectDatabase with in-memory system should never fail");

    let file_path = SystemPathBuf::from(USER_FILE);
    let user_file = system_path_to_file(&db, &file_path)
        .expect("user.py should be resolvable after writing it to the in-memory filesystem");

    db.project().set_included_paths(&mut db, vec![file_path]);

    let mut diagnostics = annotation_check(&db, level, in_repl);
    // Filter out unresolved-import errors for the spython library module,
    // which is frozen into the binary and not visible to ty's resolver.
    diagnostics.extend(db.check().into_iter().filter(|d| {
        !(d.id().as_str() == "unresolved-import" && d.primary_message().contains("spython"))
    }));
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
pub use checker::Level;

pub fn annotation_check(db: &ProjectDatabase, level: Level, in_repl: bool) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in &db.project().files(db) {
        // Skip library code (Lib/spython/) — only check student files.
        let is_spython_library = file
            .path(db)
            .as_system_path()
            .is_some_and(|p| p.as_str().contains("/Lib/spython/"));
        if !is_spython_library {
            diagnostics.extend(checker::check_file_annotations(db, file, level, in_repl));
        }
    }
    diagnostics
}

/// Convert an absolute filesystem path (Unix `/foo/bar` or Windows
/// `C:\foo\bar`) into a POSIX-style path rooted at `/` that `InMemorySystem`
/// will accept as its current directory. MemoryFileSystem asserts `cwd`
/// starts with `/`, which a Windows drive-letter absolute path does not.
/// Applying the same transform to every sibling preserves relative layout,
/// so imports still resolve.
fn virtualize_path(path: &SystemPath) -> SystemPathBuf {
    let normalized = path.as_str().replace('\\', "/");
    let stripped = match normalized.as_bytes() {
        [b, b':', ..] if b.is_ascii_alphabetic() => &normalized[2..],
        _ => normalized.as_str(),
    };
    let mut out = String::with_capacity(stripped.len() + 1);
    if !stripped.starts_with('/') {
        out.push('/');
    }
    out.push_str(stripped);
    SystemPathBuf::from(out)
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
    let parsed = parsed_module(db, file);
    let module = parsed.load(db);
    let source = ruff_db::source::source_text(db, file);
    let source_str = source.as_str();

    let extraction = doctests::extract_doctests(module.syntax(), source_str);

    let mut diagnostics = doctests::malformed_prompt_diagnostics(file, &extraction.malformed);

    if extraction.snippets.is_empty() {
        return diagnostics;
    }

    let (syn_source, syn_map) = doctests::build_synthetic_source(source_str, &extraction.snippets);

    // Mirror the original file (and its transitive first-party imports) into
    // an in-memory filesystem under POSIX-virtual paths so ty's resolver can
    // follow `from helper import …` references from inside doctests. We
    // virtualize because `InMemorySystem`'s MemoryFileSystem asserts that its
    // cwd starts with `/`, which a Windows absolute path like `C:/…` does not.
    let Some(original_path) = file.path(db).as_system_path() else {
        return diagnostics;
    };
    let original_virtual = virtualize_path(original_path);
    let cwd = original_virtual
        .parent()
        .map(SystemPath::to_path_buf)
        .unwrap_or_else(|| SystemPathBuf::from(PROJECT_ROOT));
    let system = InMemorySystem::new(cwd.clone());

    let mut transitive: HashSet<File> = HashSet::new();
    collect_import_files(db, file, &mut transitive);
    for dep in &transitive {
        if *dep == file {
            continue;
        }
        let Some(dep_path) = dep.path(db).as_system_path() else {
            continue;
        };
        let dep_src = ruff_db::source::source_text(db, *dep);
        let _ = system.write_file(&virtualize_path(dep_path), dep_src.as_str());
    }

    // Write the synthetic source at the virtualized original file path.
    system
        .write_file(&original_virtual, &syn_source)
        .expect("writing to in-memory filesystem should never fail");

    let metadata = ProjectMetadata::new(Name::new("spython"), cwd);
    let mut scratch = ProjectDatabase::new(metadata, system)
        .expect("building ProjectDatabase with in-memory system should never fail");
    let syn_file = system_path_to_file(&scratch, &original_virtual)
        .expect("synthetic file should be resolvable after writing it");
    scratch
        .project()
        .set_included_paths(&mut scratch, vec![original_virtual]);

    let mut raw = annotation_check(&scratch, level, false);
    raw.extend(scratch.check().into_iter().filter(|d| {
        !(d.id().as_str() == "unresolved-import" && d.primary_message().contains("spython"))
    }));

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
    let parsed = parsed_module(db, file);
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
        register_ffi_module(vm);
        vm.insert_sys_path(vm.new_pyobj(parent_dir.as_str()))?;
        vm.run_string(scope, &source, filename).map(drop)
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
/// Checks `accumulated + "\n" + new_input` for cross-reference correctness.
/// If errors are found, prints them with line numbers adjusted to be relative
/// to `new_input` (not the accumulated history).
///
/// Returns `true` if the input passed type checking, `false` if errors were found.
pub fn type_check_repl_input(
    accumulated: &str,
    new_input: &str,
    level: Level,
    use_color: bool,
) -> bool {
    let accumulated = accumulated.trim_end_matches('\n');
    let new_input = new_input.trim_end_matches('\n');
    let combined = if accumulated.is_empty() {
        new_input.to_owned()
    } else {
        format!("{accumulated}\n{new_input}")
    };

    let te = match type_check_source(&combined, level, true) {
        Ok(_) => return true, // warnings don't block execution
        Err(te) => te,
    };

    // Calculate the line offset: number of lines in accumulated source.
    let line_offset = if accumulated.is_empty() {
        0
    } else {
        accumulated.lines().count()
    };

    let new_input_lines: Vec<&str> = new_input.lines().collect();
    print_repl_diagnostics(
        &te.diagnostics,
        &combined,
        line_offset,
        &new_input_lines,
        use_color,
    );

    false
}

/// Infer the static type of a Python expression in the REPL context.
///
/// Builds a source with `accumulated + "\n" + expr`, type-checks it, and
/// returns the display string of the inferred type for the expression.
pub fn infer_expression_type(accumulated: &str, expr: &str) -> Option<String> {
    // Wrap the expression as the last statement so we can find it in the AST.
    let source = if accumulated.is_empty() {
        expr.to_owned()
    } else {
        format!("{}\n{}", accumulated.trim_end_matches('\n'), expr)
    };

    let cwd = SystemPathBuf::from(PROJECT_ROOT);
    let system = InMemorySystem::new(cwd.clone());
    system
        .write_file(SystemPath::new(USER_FILE), &source)
        .ok()?;

    let metadata = ProjectMetadata::new(Name::new("spython"), cwd);
    let mut db = ProjectDatabase::new(metadata, system).ok()?;

    let file_path = SystemPathBuf::from(USER_FILE);
    let file = system_path_to_file(&db, &file_path).ok()?;
    db.project().set_included_paths(&mut db, vec![file_path]);

    // Parse and find the last expression statement.
    let parsed = parsed_module(&db, file);
    let module = parsed.load(&db);
    let stmts = module.suite();
    let last_stmt = stmts.last()?;

    // The last statement should be an expression statement.
    let expr_node = match last_stmt {
        Stmt::Expr(expr_stmt) => &*expr_stmt.value,
        _ => return None,
    };

    let model = SemanticModel::new(&db, file);
    let ty = ExprRef::from(expr_node).inferred_type(&model)?;
    let type_str = ty.display(&db).to_string();
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

/// Print diagnostics with line numbers adjusted for REPL context.
fn print_repl_diagnostics(
    diagnostics: &[Diagnostic],
    combined_source: &str,
    line_offset: usize,
    new_input_lines: &[&str],
    use_color: bool,
) {
    use ruff_db::diagnostic::Severity;
    let mut error_count = 0;

    for diag in diagnostics {
        let is_error = diag.severity() == Severity::Error;
        if is_error {
            error_count += 1;
        }

        // Get the line number from the primary span's byte offset into combined source.
        let (line, col, span_len) = diag
            .primary_span_ref()
            .and_then(|span| span.range())
            .map(|range| {
                let offset = usize::from(range.start()).min(combined_source.len());
                let line = combined_source[..offset].matches('\n').count();
                let line_start = combined_source[..offset].rfind('\n').map_or(0, |i| i + 1);
                let col = offset - line_start;
                let len = usize::from(range.end()) - usize::from(range.start());
                (line, col, len)
            })
            .unwrap_or((0, 0, 1));

        // Adjust line number relative to new input.
        let adjusted_line = line.saturating_sub(line_offset);
        let display_line = adjusted_line + 1; // 1-indexed

        let severity = match (is_error, use_color) {
            (true, true) => "\x1b[1;91merror",
            (true, false) => "error",
            (false, true) => "\x1b[1;93mwarning",
            (false, false) => "warning",
        };

        let id = diag.id();
        let msg = diag.primary_message();
        let reset = if use_color { "\x1b[0m" } else { "" };
        let bold = if use_color { "\x1b[1m" } else { "" };

        eprintln!("{severity}[{id}]{reset}: {bold}{msg}{reset}");
        eprintln!(" --> user.py:{display_line}:{}", col + 1);

        // Show the source line if available.
        if adjusted_line < new_input_lines.len() {
            let src_line = new_input_lines[adjusted_line];
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

// --- REPL state ---

/// Persistent Python interpreter state for the web REPL.
pub struct ReplState {
    // `scope` is wrapped in `Option` so the `Drop` impl can take it and
    // drop it inside the VM context (see below).
    scope: Option<vm::scope::Scope>,
    interp: Option<vm::Interpreter>,
    /// Accumulated source of all successfully type-checked and executed inputs.
    accumulated_source: String,
    /// Teaching level for annotation/construct checking.
    level: Level,
}

#[cfg(test)]
static REPL_ATEXIT_RAN: AtomicBool = AtomicBool::new(false);

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

    /// The accumulated source of all successfully type-checked and executed inputs.
    pub fn accumulated_source(&self) -> &str {
        &self.accumulated_source
    }

    /// The teaching level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Append successfully executed source to the accumulator.
    pub fn append_source(&mut self, source: &str) {
        if !self.accumulated_source.is_empty() {
            self.accumulated_source.push('\n');
        }
        self.accumulated_source.push_str(source);
    }

    /// Run the REPL loop inside the VM context.
    ///
    /// The closure receives the VM and the scope. This is needed by the CLI
    /// REPL to create the rustyline `Editor` with a `ReplHelper` that borrows
    /// the VM.
    pub fn enter<R>(&self, f: impl FnOnce(&vm::VirtualMachine, vm::scope::Scope) -> R) -> R {
        let scope = self.scope.as_ref().expect("scope is live").clone();
        self.interp
            .as_ref()
            .expect("interpreter is live")
            .enter(|vm| f(vm, scope))
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
            "<init>".to_owned(),
        ) {
            vm.print_exception(exc);
        }
        if has_source {
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
    let accumulated_source = if has_source {
        source.to_owned()
    } else {
        String::new()
    };
    Box::new(ReplState {
        scope: Some(scope),
        interp: Some(interp),
        accumulated_source,
        level,
    })
}

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

    let text_width_fn = vm.new_function(
        "text_width",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            vm.ctx
                .new_float(wasm_ffi::measure_text_width(&text, &font_css))
                .into()
        },
    );

    let text_height_fn = vm.new_function(
        "text_height",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            vm.ctx
                .new_float(wasm_ffi::measure_text_height(&text, &font_css))
                .into()
        },
    );

    let text_x_offset_fn = vm.new_function(
        "text_x_offset",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            vm.ctx
                .new_float(wasm_ffi::measure_text_x_offset(&text, &font_css))
                .into()
        },
    );

    let text_y_offset_fn = vm.new_function(
        "text_y_offset",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            vm.ctx
                .new_float(wasm_ffi::measure_text_y_offset(&text, &font_css))
                .into()
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

    let dict = vm.ctx.new_dict();
    dict.set_item("show_svg", show_svg_fn.into(), vm).unwrap();
    dict.set_item("get_key_event", get_key_event_fn.into(), vm)
        .unwrap();
    dict.set_item("text_width", text_width_fn.into(), vm)
        .unwrap();
    dict.set_item("text_height", text_height_fn.into(), vm)
        .unwrap();
    dict.set_item("text_x_offset", text_x_offset_fn.into(), vm)
        .unwrap();
    dict.set_item("text_y_offset", text_y_offset_fn.into(), vm)
        .unwrap();
    dict.set_item("load_bitmap", load_bitmap_fn.into(), vm)
        .unwrap();
    #[cfg(test)]
    dict.set_item("__test_mark_atexit_ran", test_mark_atexit_ran_fn.into(), vm)
        .unwrap();
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

/// Execute one REPL expression/statement in the session's scope.
///
/// Returns `REPL_OK` on success, `REPL_ERROR` on type/runtime error, or
/// `REPL_QUIT` if the user called `exit()` / `quit()` (SystemExit raised).
pub fn repl_run(state: &mut ReplState, code: &str) -> u32 {
    // Handle :type / :time commands (works in both CLI and WASM).
    let trimmed = code.trim();
    if let Some(expr) = trimmed.strip_prefix(":type ") {
        match infer_expression_type(&state.accumulated_source, expr) {
            Some(ty) => println!("{ty}"),
            None => eprintln!("Could not infer type"),
        }
        return REPL_OK;
    }
    if trimmed == ":type" {
        eprintln!("Usage: :type <expression>");
        return REPL_OK;
    }

    if trimmed == ":time" {
        eprintln!("Usage: :time <expression>");
        return REPL_OK;
    }

    let (code, timed) = if let Some(expr) = trimmed.strip_prefix(":time ") {
        if expr.trim().is_empty() {
            eprintln!("Usage: :time <expression>");
            return REPL_OK;
        }
        (format!("{expr}\n"), true)
    } else {
        (code.to_owned(), false)
    };

    if !type_check_repl_input(&state.accumulated_source, &code, state.level, true) {
        return REPL_ERROR;
    }

    let scope = state.scope.as_ref().unwrap().clone();
    let start = timed.then(std::time::Instant::now);
    let result = state
        .interp
        .as_ref()
        .expect("interpreter is live")
        .enter(|vm| {
            match vm
                .compile(&code, vm::compiler::Mode::Single, "<stdin>".to_owned())
                .map_err(|err| vm.new_syntax_error(&err, Some(&code)))
                .and_then(|code_obj| vm.run_code_obj(code_obj, scope))
            {
                Ok(_) => REPL_OK,
                Err(exc) => {
                    if exc.fast_isinstance(vm.ctx.exceptions.system_exit) {
                        REPL_QUIT
                    } else {
                        vm.print_exception(exc);
                        REPL_ERROR
                    }
                }
            }
        });

    if result == REPL_OK {
        state.append_source(&code);
        if let Some(start) = start {
            println!("Time: {}", format_elapsed(start.elapsed()));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
