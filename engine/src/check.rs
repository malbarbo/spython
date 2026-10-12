//! The type checker: the ty project spython builds around a file or a source
//! string, the checks it runs over it, and the printing of what it found.

use std::collections::HashSet;
use std::panic::RefUnwindSafe;

use ruff_db::Db as SourceDb;
use ruff_db::diagnostic::{
    Annotation, Diagnostic, DisplayDiagnosticConfig, DisplayDiagnostics, Span, SubDiagnostic,
    UnifiedFile,
};
use ruff_db::files::{File, system_path_to_file};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{InMemorySystem, MemoryFileSystem, System, SystemPath, SystemPathBuf};
use ruff_python_ast::name::Name;
use ruff_python_ast::statement_visitor::{StatementVisitor, walk_stmt};
use ruff_python_ast::{ExprRef, Stmt};
use ruff_ranged_value::RangedValue;
use ruff_text_size::{TextRange, TextSize};
use ty_module_resolver::{
    FallibleStrategy, ImportingFile, ModuleName, SearchPathSettings, resolve_module,
};
use ty_project::metadata::Options;
use ty_project::metadata::options::{EnvironmentOptions, Rules};
use ty_project::metadata::python_version::SupportedPythonVersion;
use ty_project::{Db, ProjectDatabase, ProjectMetadata};
use ty_python_core::{global_scope, place_table};
use ty_python_semantic::Db as _;
use ty_python_semantic::lint::Level as TyLevel;
use ty_python_semantic::types::Type;
use ty_python_semantic::{HasType, SemanticModel};

use crate::{Level, checker, doctests, spython_lib};

/// What only a check over the file system needs, which the page never runs.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::check_file;

/// The directory of [`USER_FILE`], which no search path includes. The code
/// in the REPL and on the page is not a module at run time, so no import
/// may find it, and the directory holds nothing else an import could find.
const SOURCE_DIR: &str = "/source";
const USER_FILE: &str = "/source/user.py";
/// The REPL input alone, which a type error of the REPL points into.
const INPUT_FILE: &str = "/source/<repl>";
/// Where the modules a session started from go. A root of their own, so no
/// file the student wrote can land on [`USER_FILE`] and quietly replace the
/// code being checked.
const MODULE_ROOT: &str = "/modules";
/// Where the spython library goes. It is an extra path, and the parent
/// directory is on no search path, so `import _spython_lib` finds nothing.
const LIB_PARENT: &str = "/";
/// Errors returned when type checking finds problems in source code.
pub struct TypeErrors {
    pub db: ProjectDatabase,
    pub diagnostics: Vec<Diagnostic>,
}

/// What a check found that does not stop the file, with the database that
/// prints it. A check that found nothing carries an empty one.
pub struct Warnings {
    pub db: ProjectDatabase,
    pub diagnostics: Vec<Diagnostic>,
}

impl Warnings {
    /// Print what the check found, and nothing when it found nothing.
    pub fn show(&self, use_color: bool) {
        print_type_errors(&self.db, &self.diagnostics, use_color);
    }
}
/// A project rooted at `cwd`, over `system`, with spython's rules applied.
///
/// Promotes a few ty rules from `Ignore`/`Warn` to `Error` for the teaching
/// context: `possibly-unresolved-reference` (variable assigned only in some
/// branches) and `division-by-zero` (literal `/ 0`, `% 0`, etc.).
///
/// `roots` are the directories that hold the student's own modules, in the
/// order the interpreter looks in them. `extra_paths` come before them, and
/// hold the embedded spython library so `from spython import ...` resolves.
/// After them comes the standard library and nothing else.
pub fn project_database<S>(
    cwd: SystemPathBuf,
    roots: Vec<SystemPathBuf>,
    extra_paths: Vec<SystemPathBuf>,
    system: S,
) -> Result<ProjectDatabase, String>
where
    S: System + 'static + Send + Sync + RefUnwindSafe,
{
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
    let environment = EnvironmentOptions {
        // The version RustPython runs. Without it, ty takes the version of
        // whatever Python it finds on the machine, and a 3.13 there rejects
        // an annotation that names a class defined later, which 3.14 runs.
        python_version: Some(RangedValue::cli(SupportedPythonVersion::Py314)),
        ..EnvironmentOptions::default()
    };
    metadata.apply_override_options(Options {
        rules: Some(rules),
        environment: Some(environment),
        ..Options::default()
    });
    // ty looks for a Python installation on the machine and fails when
    // `VIRTUAL_ENV` names one that is not there. The search paths below
    // replace whatever it found, so a failure there does not matter.
    let mut db = ProjectDatabase::use_defaults(metadata, system);
    // Left to itself, ty also looks in `src` and `python` and in the
    // site-packages of the installation. The interpreter does neither.
    let search_paths = SearchPathSettings {
        extra_paths,
        src_roots: roots,
        ..SearchPathSettings::empty()
    }
    .to_search_paths(
        SourceDb::system(&db),
        SourceDb::vendored(&db),
        &FallibleStrategy,
    )
    .map_err(|err| err.to_string())?;
    let mut settings = db.project().program_settings(&db).clone();
    settings.search_paths = search_paths;
    db.project().update_program(&mut db, settings);
    Ok(db)
}
/// A project over an in-memory filesystem holding `source` as [`USER_FILE`],
/// the spython library, and `modules`.
///
/// `modules` are the first-party files the source imports, each a path
/// relative to the directory the source came from and its text. Nothing on
/// the real filesystem is reachable from here, so `import helpers` only
/// resolves if `helpers.py` is among them. They go under a root of their
/// own, out of reach of the source's own path, and are not included, so the
/// checker reads them to resolve names and reports nothing about them.
fn in_memory_project(source: &str, modules: &[(String, String)]) -> (ProjectDatabase, File) {
    in_memory_project_at(SystemPath::new(USER_FILE), source, modules)
}
/// The [`in_memory_project`] that holds `source` at `file_path`. A path under
/// [`MODULE_ROOT`] makes `source` one of the modules, so its imports resolve
/// as they do for the file it stands for, the relative ones included.
fn in_memory_project_at(
    file_path: &SystemPath,
    source: &str,
    modules: &[(String, String)],
) -> (ProjectDatabase, File) {
    let cwd = SystemPathBuf::from(SOURCE_DIR);
    let system = InMemorySystem::from_memory_fs(MemoryFileSystem::with_current_directory(&cwd));
    system
        .fs()
        .write_file_all(file_path, source)
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
    let lib_root = spython_lib::write_into(&system, SystemPath::new(LIB_PARENT));

    let mut roots = Vec::new();
    if !modules.is_empty() || file_path.starts_with(MODULE_ROOT) {
        roots.push(SystemPathBuf::from(MODULE_ROOT));
    }
    let mut db = project_database(cwd, roots, vec![lib_root], system)
        .expect("building ProjectDatabase with in-memory system should never fail");

    let user_file = system_path_to_file(&db, file_path)
        .expect("the source should be resolvable after writing it to the in-memory filesystem");

    db.project()
        .set_included_paths(&mut db, vec![file_path.to_path_buf()]);
    (db, user_file)
}
/// Type-check `source` entirely in memory (no OS filesystem access).
///
/// Used by the WASM shim, which has one file and nothing to import.
pub fn check_source(
    source: &str,
    level: Level,
    in_repl: bool,
) -> Result<Warnings, Box<TypeErrors>> {
    check_in_memory(source, level, in_repl.then_some(TextSize::default()), &[])
}
/// Type-check `source` against `modules`, the first-party files it imports.
/// `repl_input` is where the REPL input starts in `source`, and `None` for a
/// file.
fn check_in_memory(
    source: &str,
    level: Level,
    repl_input: Option<TextSize>,
    modules: &[(String, String)],
) -> Result<Warnings, Box<TypeErrors>> {
    let (db, user_file) = in_memory_project(source, modules);

    let mut diagnostics = annotation_check(&db, level, repl_input);
    diagnostics.extend(db.check());
    // Doctests are code too, so validate the prompts and type-check the
    // snippets.
    // Skip in REPL context, where incremental input doesn't have docstrings.
    if repl_input.is_none() {
        diagnostics.extend(check_file_doctests(
            &db,
            user_file,
            SystemPath::new(SOURCE_DIR),
            level,
        ));
    }

    let has_errors = diagnostics
        .iter()
        .any(|d| d.severity() == ruff_db::diagnostic::Severity::Error);
    if has_errors {
        Err(Box::new(TypeErrors { db, diagnostics }))
    } else {
        Ok(Warnings { db, diagnostics })
    }
}
/// Run the annotation checker on all files registered in `db`.
pub fn annotation_check(
    db: &ProjectDatabase,
    level: Level,
    repl_input: Option<TextSize>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in user_files(db, db.project().files(db).into_iter()) {
        diagnostics.extend(checker::check_file_annotations(db, file, level, repl_input));
    }
    diagnostics
}

/// The files among `files` the student wrote, sorted by path. The embedded
/// spython library is not one of them. The sets these files come from are
/// backed by hashes, whose order is not the same from one platform to the
/// next, and the diagnostics follow that order into the snapshot tests.
fn user_files(db: &ProjectDatabase, files: impl Iterator<Item = File>) -> Vec<File> {
    let mut files: Vec<File> = files
        .filter(|f| {
            !f.path(db)
                .as_system_path()
                .is_some_and(spython_lib::is_lib_path)
        })
        .collect();
    files.sort_by_key(|f| f.path(db).as_str().to_owned());
    files
}
/// The files among `files` that a file in `root` can name, keyed by their
/// path relative to `root`. [`in_memory_project`] takes its modules in that
/// form.
///
/// An in-memory check cannot reach the real directory, so what the source
/// imports has to travel as text. A file outside `root` is left out: there
/// is no relative name that would reach it, so writing it would not help the
/// resolver either. Sorted, because `files` often comes from a `HashSet`.
pub fn modules_beside(
    db: &ProjectDatabase,
    root: &SystemPath,
    files: impl IntoIterator<Item = File>,
) -> Vec<(String, String)> {
    let mut modules: Vec<(String, String)> = files
        .into_iter()
        .filter_map(|f| {
            let relative = relative_path(db, f, root)?;
            let source = ruff_db::source::source_text(db, f);
            Some((relative, source.to_string()))
        })
        .collect();
    modules.sort();
    modules
}
/// The path of `file` relative to `root`, or `None` when `file` is not
/// under `root`.
fn relative_path(db: &ProjectDatabase, file: File, root: &SystemPath) -> Option<String> {
    let path = file.path(db).as_system_path()?;
    Some(path.strip_prefix(root).ok()?.as_str().to_owned())
}
/// Type-check and format-check doctests inside a file's docstrings.
///
/// Returns diagnostics for:
/// * malformed lines (`>>>x`, `...x`, a line to the left of its `>>>`) and
///   doctests inside a function, which never run. The check always reports these;
/// * annotation rules / level restrictions / ty type errors found inside the
///   doctest snippets, remapped to the original docstring locations.
///
/// Doctest diagnostics complement the ordinary file-level check; callers
/// should append this to their existing diagnostic list.
///
/// `root` is the directory of the main file, which the imports of every file
/// resolve from.
pub fn check_file_doctests(
    db: &ProjectDatabase,
    file: File,
    root: &SystemPath,
    level: Level,
) -> Vec<Diagnostic> {
    let parsed = parsed_module(db, db.program_file(file).python_file(db));
    let module = parsed.load(db);
    let source = ruff_db::source::source_text(db, file);
    let source_str = source.as_str();

    let extraction = doctests::extract_doctests(module.syntax(), source_str);

    let mut diagnostics = doctests::malformed_diagnostics(file, &extraction.malformed);
    diagnostics.extend(doctests::never_run_diagnostics(file, &extraction.never_run));
    diagnostics.extend(doctests::test_dict_diagnostics(file, &extraction.test_dict));
    diagnostics.extend(doctests::under_name_check_diagnostics(
        file,
        &extraction.under_name_check,
    ));

    if extraction.snippets.is_empty() {
        return diagnostics;
    }

    let module_names: HashSet<&str> = place_table(db, global_scope(db, db.program_file(file)))
        .symbols()
        .filter(|symbol| symbol.is_bound())
        .map(|symbol| symbol.name().as_str())
        .collect();
    let (syn_source, syn_map) =
        doctests::build_synthetic_source(source_str, &extraction.snippets, &module_names);

    // A doctest can name what the file around it imports, so the check gets
    // the file's first-party imports as the modules to resolve against --
    // the same in-memory project the REPL checks its inputs in. The doctests
    // take the place of the file, under the same path.
    let mut transitive: HashSet<File> = HashSet::new();
    collect_import_files(db, file, &mut transitive);
    transitive.remove(&file);
    let modules = modules_beside(db, root, transitive);

    let (scratch, syn_file) = match relative_path(db, file, root) {
        Some(relative) => in_memory_project_at(
            &SystemPathBuf::from(format!("{MODULE_ROOT}/{relative}")),
            &syn_source,
            &modules,
        ),
        None => in_memory_project(&syn_source, &modules),
    };

    let mut raw = annotation_check(&scratch, level, None);
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
    visit_imports(db, file, module.suite(), seen);

    // A doctest imports from where its file does, when `spython test` runs it.
    let source = ruff_db::source::source_text(db, file);
    for snippet in doctests::extract_doctests(module.syntax(), source.as_str()).snippets {
        if let Ok(snippet) = ruff_python_parser::parse_module(&snippet.code) {
            visit_imports(db, file, snippet.suite(), seen);
        }
    }
}
/// Collect the first-party files that the imports in `body` name, with the
/// files they import, as [`collect_import_files`] does.
fn visit_imports(db: &ProjectDatabase, file: File, body: &[Stmt], seen: &mut HashSet<File>) {
    let mut imports = Imports(Vec::new());
    imports.visit_body(body);
    for stmt in imports.0 {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    if is_library(&alias.name.id) {
                        continue;
                    }
                    if let Some(name) = ModuleName::new(alias.name.id.as_str()) {
                        visit_module(db, file, &name, seen);
                    }
                }
            }
            Stmt::ImportFrom(import_from) => {
                let module_str = import_from.module.as_deref();
                if module_str.is_some_and(is_library) {
                    continue;
                }
                if let Ok(name) = ModuleName::from_identifier_parts(
                    db,
                    importing_file(db, file),
                    module_str,
                    import_from.level,
                ) {
                    visit_module(db, file, &name, seen);
                    // `from pkg import util` can name a module as well as a
                    // name defined in `pkg`.
                    for alias in &import_from.names {
                        if let Some(child) = ModuleName::new(alias.name.id.as_str()) {
                            let mut submodule = name.clone();
                            submodule.extend(&child);
                            visit_module(db, file, &submodule, seen);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
/// The import statements of a module, including the ones inside a function
/// or a block. The interpreter runs those too.
struct Imports<'a>(Vec<&'a Stmt>);

impl<'a> StatementVisitor<'a> for Imports<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if matches!(stmt, Stmt::Import(_) | Stmt::ImportFrom(_)) {
            self.0.push(stmt);
        } else {
            walk_stmt(self, stmt);
        }
    }
}
/// Returns `true` if `module` is the spython library or one of its modules,
/// `false` otherwise. The library is not student code, but a student module
/// can still be named `spython_helpers`.
fn is_library(module: &str) -> bool {
    module == "spython" || module.starts_with("spython.")
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
/// Print type errors to stderr. Pass `use_color = true` in WASM (ansi.ts renders
/// the ANSI codes), or `stderr().is_terminal()` from the CLI binary.
pub fn print_type_errors(db: &ProjectDatabase, diagnostics: &[Diagnostic], use_color: bool) {
    use ruff_db::diagnostic::Severity;
    use std::fmt::Write;
    if diagnostics.is_empty() {
        return;
    }
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
/// resolving imports against `modules`, the first-party files the session
/// started from, which are not on any filesystem the checker can see.
/// Diagnostics about the input are printed with their line numbers relative
/// to it; diagnostics about the accumulated source are dropped.
///
/// Returns `true` if the input passed type checking, `false` if errors were found.
pub(crate) fn type_check_repl_input(
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

    // Where the input starts in `combined`. What lands before it is about the
    // accumulated source, which no prompt can reach: printing it would blame
    // what the user just typed for code they cannot fix from here.
    let input_offset = combined.len() - new_input.len();
    let input_start = TextSize::try_from(input_offset).unwrap_or(TextSize::new(u32::MAX));

    let te = match check_in_memory(&combined, level, Some(input_start), modules) {
        Ok(_) => return true, // warnings don't block execution
        Err(te) => te,
    };
    let user_file = system_path_to_file(&te.db, SystemPath::new(USER_FILE))
        .expect("the user file was written before the check");
    let input_path = SystemPath::new(INPUT_FILE);
    te.db
        .system()
        .as_writable()
        .expect("the in-memory system is writable")
        .write_file(input_path, new_input)
        .expect("writing to in-memory filesystem should never fail");
    let input_file =
        system_path_to_file(&te.db, input_path).expect("the input file was just written");
    let input = Input {
        user_file,
        input_file,
        range: TextRange::new(input_start, TextSize::of(combined.as_str())),
    };
    let in_input: Vec<Diagnostic> = te
        .diagnostics
        .iter()
        .filter(|diag| match diagnostic_start(diag) {
            // No range means the diagnostic is about the check as a whole,
            // not about a line of history, so it stays.
            None => true,
            Some(start) => input.range.contains_inclusive(start),
        })
        .map(|diag| input.move_onto(diag))
        .collect();
    if !in_input
        .iter()
        .any(|d| d.severity() == ruff_db::diagnostic::Severity::Error)
    {
        return true;
    }

    print_type_errors(&te.db, &in_input, use_color);

    false
}
/// Infer the static type of a Python expression in the REPL context.
///
/// Builds a source with `accumulated + "\n" + expr`, type-checks it against
/// `modules` the way [`type_check_repl_input`] does, and returns the display
/// string of the inferred type for the expression.
pub(crate) fn infer_expression_type(
    accumulated: &str,
    expr: &str,
    modules: &[(String, String)],
) -> Option<String> {
    let source = if accumulated.is_empty() {
        expr.to_owned()
    } else {
        format!("{}\n{}", accumulated.trim_end_matches('\n'), expr)
    };

    let (db, file) = in_memory_project(&source, modules);

    let program_file = db.program_file(file);
    let parsed = parsed_module(&db, program_file.python_file(&db));
    let module = parsed.load(&db);
    // The last statement belongs to the history when `expr` holds none.
    let expr_start = TextSize::try_from(source.len() - expr.len()).ok()?;
    let expr_node = match module.suite().last()? {
        Stmt::Expr(expr_stmt) if expr_stmt.range.start() == expr_start => &*expr_stmt.value,
        _ => return None,
    };

    let model = SemanticModel::new(&db, program_file);
    let env = model.program_environment();
    let ty = ExprRef::from(expr_node).inferred_type(&model)?;
    // A student wants `int` for `1`, but a function would show as a
    // `Callable` instead of its `def` once promoted.
    let ty = match ty {
        Type::FunctionLiteral(_) => ty,
        _ => ty.promote(&db, &env),
    };
    Some(ty.display(&db, &env).to_string())
}
/// Where a diagnostic starts.
///
/// `None` for one that carries no range. ty reports an unreadable file that
/// way. It is nowhere in particular, which is not the same as being at the
/// start, and the caller has to keep the two apart.
fn diagnostic_start(diag: &Diagnostic) -> Option<TextSize> {
    diag.primary_span()
        .and_then(|span| span.range())
        .map(|range| range.start())
}
/// The REPL input inside the source the REPL checks, which puts the code of
/// the session before it, and the file that holds the input alone.
struct Input {
    user_file: File,
    input_file: File,
    range: TextRange,
}

impl Input {
    /// A copy of `diag` with its annotations moved from the input in the
    /// checked source onto the input file, so the error shows the lines and
    /// columns of what the user typed. An annotation that points into the
    /// session before the input goes, as the user cannot fix that code from
    /// the prompt, and so does a sub-diagnostic that only pointed there. An
    /// annotation without a range moves to the input file without one, and an
    /// annotation that points into another file stays.
    fn move_onto(&self, diag: &Diagnostic) -> Diagnostic {
        let mut moved = Diagnostic::new(diag.id(), diag.severity(), diag.headline_message());
        moved.set_documentation_url(diag.documentation_url().map(str::to_owned));
        for annotation in diag.annotations() {
            if let Some(annotation) = self.move_annotation(annotation) {
                moved.annotate(annotation);
            }
        }
        for sub in diag.sub_diagnostics() {
            let mut moved_sub = SubDiagnostic::new(sub.severity(), sub.headline_message());
            for annotation in sub.annotations() {
                if let Some(annotation) = self.move_annotation(annotation) {
                    moved_sub.annotate(annotation);
                }
            }
            if sub.annotations().is_empty() || !moved_sub.annotations().is_empty() {
                moved.sub(moved_sub);
            }
        }
        moved
    }

    fn move_annotation(&self, annotation: &Annotation) -> Option<Annotation> {
        let span = annotation.get_span();
        if span.file() != &UnifiedFile::Ty(self.user_file) {
            return Some(annotation.clone());
        }
        let range = match span.range() {
            None => None,
            Some(range) if self.range.contains_range(range) => Some(range - self.range.start()),
            Some(_) => return None,
        };
        let mut annotation = annotation.clone();
        annotation.set_span(Span::from(self.input_file).with_optional_range(range));
        Some(annotation)
    }
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
    fn an_import_finds_nothing_the_session_does_not_run() {
        for import in [
            "import _spython_lib",
            "import modules",
            "import source",
            "import user",
        ] {
            assert!(
                !type_check_repl_input("", import, Level::Full, false, &helpers()),
                "{import}"
            );
            assert!(
                check_source(import, Level::Full, false).is_err(),
                "{import}"
            );
        }
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
    fn input_can_replace_a_function_of_an_earlier_input() {
        assert!(type_check_repl_input(
            "def f() -> int:\n    return 1\n",
            "def f() -> int:\n    return 2\n",
            Level::Functions,
            false,
            &[],
        ));
    }

    #[test]
    fn a_function_defined_twice_in_one_input_is_still_an_error() {
        assert!(!type_check_repl_input(
            "x: int = 1",
            "def f() -> int:\n    return 1\n\ndef f() -> int:\n    return 2\n",
            Level::Functions,
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

    #[test]
    fn a_type_shows_the_class_of_each_literal_in_it() {
        let session = "b = len(input()) > 0";
        for (expr, expected) in [
            ("1", "int"),
            ("1 if b else \"a\"", "int | str"),
            ("True if b else 1", "int"),
            ("\"a\" if b else None", "str | None"),
            ("(1, \"a\")", "tuple[int, str]"),
            ("b\"x\"", "bytes"),
        ] {
            assert_eq!(
                infer_expression_type(session, expr, &[]).as_deref(),
                Some(expected),
                "{expr}"
            );
        }
    }

    #[test]
    fn a_function_shows_as_its_def() {
        assert_eq!(
            infer_expression_type("", "len", &[]).as_deref(),
            Some("def len(obj: Sized, /) -> int")
        );
    }

    #[test]
    fn a_text_with_no_statement_gets_no_type_from_the_history() {
        assert_eq!(infer_expression_type("\"abc\"", "# c", &[]), None);
    }
}
