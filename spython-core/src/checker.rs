//! Annotation checker for spython.
//!
//! This module provides annotation checking that runs after ty's type checking
//! and generates diagnostics using ty's diagnostic system.

use ruff_db::diagnostic::{Annotation, Diagnostic, DiagnosticId, Severity, Span};
use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_python_ast::{Expr, Stmt, StmtAssign, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;
use ty_project::Db;
use ty_python_semantic::lint::lint_documentation_url;

use crate::lints::{
    MISSING_ATTRIBUTE_ANNOTATION, MISSING_PARAMETER_ANNOTATION, MISSING_RETURN_ANNOTATION,
};

/// Check a file for missing annotations.
///
/// Returns a vector of diagnostics for any missing annotations found.
pub fn check_file_annotations(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let parsed = parsed_module(db, file);
    let module = parsed.load(db);

    // Check all statements in the module
    check_stmts(module.suite(), file, &mut diagnostics, false);

    diagnostics
}

/// Recursively check a list of statements for missing annotations.
fn check_stmts(stmts: &[Stmt], file: File, diagnostics: &mut Vec<Diagnostic>, in_class: bool) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_function(func, file, diagnostics, in_class);
                // Recurse into the body
                check_stmts(&func.body, file, diagnostics, false);
            }
            Stmt::ClassDef(cls) => {
                // Check class body for unannotated assignments
                check_class_body(cls, file, diagnostics);
                // Recurse into nested definitions
                check_stmts(&cls.body, file, diagnostics, true);
            }
            _ => {}
        }
    }
}

/// Check a function definition for missing parameter and return annotations.
fn check_function(
    func: &StmtFunctionDef,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    in_class: bool,
) {
    let params = &func.parameters;

    // Skip the first positional parameter in a method (the implicit receiver: self/cls)
    let mut skip_first = in_class;

    // Check positional-only and regular positional parameters
    for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
        if skip_first {
            skip_first = false;
            continue;
        }

        if pwd.parameter.annotation.is_none() {
            diagnostics.push(make_lint_diagnostic(
                &MISSING_PARAMETER_ANNOTATION,
                file,
                pwd.parameter.range(),
                format!(
                    "Parameter `{}` is missing a type annotation",
                    pwd.parameter.name.as_str()
                ),
            ));
        }
    }

    // Check keyword-only parameters
    for pwd in &params.kwonlyargs {
        if pwd.parameter.annotation.is_none() {
            diagnostics.push(make_lint_diagnostic(
                &MISSING_PARAMETER_ANNOTATION,
                file,
                pwd.parameter.range(),
                format!(
                    "Parameter `{}` is missing a type annotation",
                    pwd.parameter.name.as_str()
                ),
            ));
        }
    }

    // Check *args parameter
    if let Some(vararg) = &params.vararg
        && vararg.annotation.is_none()
    {
        diagnostics.push(make_lint_diagnostic(
            &MISSING_PARAMETER_ANNOTATION,
            file,
            vararg.range(),
            format!(
                "Parameter `*{}` is missing a type annotation",
                vararg.name.as_str()
            ),
        ));
    }

    // Check **kwargs parameter
    if let Some(kwarg) = &params.kwarg
        && kwarg.annotation.is_none()
    {
        diagnostics.push(make_lint_diagnostic(
            &MISSING_PARAMETER_ANNOTATION,
            file,
            kwarg.range(),
            format!(
                "Parameter `**{}` is missing a type annotation",
                kwarg.name.as_str()
            ),
        ));
    }

    // Check return type annotation
    if func.returns.is_none() {
        diagnostics.push(make_lint_diagnostic(
            &MISSING_RETURN_ANNOTATION,
            file,
            func.name.range(),
            format!(
                "Function `{}` is missing a return type annotation",
                func.name.as_str()
            ),
        ));
    }
}

/// Check the direct statements of a class body for unannotated assignments.
fn check_class_body(class_def: &StmtClassDef, file: File, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in &class_def.body {
        if let Stmt::Assign(StmtAssign { targets, .. }) = stmt {
            for target in targets {
                if let Expr::Name(name) = target {
                    diagnostics.push(make_lint_diagnostic(
                        &MISSING_ATTRIBUTE_ANNOTATION,
                        file,
                        name.range(),
                        format!(
                            "Class attribute `{}` is missing a type annotation",
                            name.id.as_str()
                        ),
                    ));
                }
            }
        }
    }
}

/// Create a lint diagnostic that matches ty's format.
fn make_lint_diagnostic(
    lint: &ty_python_semantic::lint::LintMetadata,
    file: File,
    range: ruff_text_size::TextRange,
    message: String,
) -> Diagnostic {
    let lint_name = lint.name();
    let diag_id = DiagnosticId::Lint(lint_name);

    let mut diag = Diagnostic::new(diag_id, Severity::Error, message);
    diag.set_documentation_url(Some(lint_documentation_url(lint_name)));
    diag.annotate(Annotation::primary(Span::from(file).with_range(range)));
    diag.info(format!("rule `{lint_name}` is enabled by default"));
    diag
}
