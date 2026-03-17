//! Annotation checker and construct restriction checker for spython.
//!
//! This module checks for missing annotations and forbidden constructs based
//! on the current teaching level.

use ruff_db::diagnostic::{Annotation, Diagnostic, DiagnosticId, Severity, Span};
use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_python_ast::{Expr, Stmt, StmtAssign, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;
use ty_project::Db;

use crate::lints::{
    FORBIDDEN_AUG_ASSIGN, FORBIDDEN_CLASS, FORBIDDEN_CLASS_METHOD, FORBIDDEN_COLLECTION_LITERAL,
    FORBIDDEN_COMPREHENSION, FORBIDDEN_CONSTRUCT, FORBIDDEN_LAMBDA, FORBIDDEN_LOOP,
    FORBIDDEN_MATCH, FORBIDDEN_SELECTION, MISSING_ATTRIBUTE_ANNOTATION,
    MISSING_PARAMETER_ANNOTATION, MISSING_RETURN_ANNOTATION,
};

/// Teaching level that controls which Python constructs are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// def, return, scalars (int, float, str, bool), string indexing.
    Functions = 0,
    /// Adds: if/elif/else.
    Selection = 1,
    /// Adds: Enum, @dataclass, match.
    Types = 2,
    /// Adds: list literals, for, while, augmented assignment.
    Arrays = 3,
    /// Adds: full classes with methods, dict/set literals, comprehensions, lambda.
    Classes = 4,
    /// Unrestricted Python (only annotations still required).
    Full = 5,
}

impl Level {
    pub fn from_u8(n: u8) -> Option<Level> {
        match n {
            0 => Some(Level::Functions),
            1 => Some(Level::Selection),
            2 => Some(Level::Types),
            3 => Some(Level::Arrays),
            4 => Some(Level::Classes),
            5 => Some(Level::Full),
            _ => None,
        }
    }
}

/// Check a file for missing annotations and forbidden constructs.
pub fn check_file_annotations(db: &dyn Db, file: File, level: Level) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let parsed = parsed_module(db, file);
    let module = parsed.load(db);

    check_stmts(module.suite(), file, &mut diagnostics, false, level);

    diagnostics
}

/// Recursively check a list of statements.
fn check_stmts(
    stmts: &[Stmt],
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    in_class: bool,
    level: Level,
) {
    for stmt in stmts {
        check_stmt(stmt, file, diagnostics, in_class, level);
    }
}

/// Check a single statement for annotations and forbidden constructs.
fn check_stmt(
    stmt: &Stmt,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    in_class: bool,
    level: Level,
) {
    match stmt {
        Stmt::FunctionDef(func) => {
            if func.is_async && level < Level::Full {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CONSTRUCT,
                    file,
                    func.range(),
                    "`async def` is not allowed".to_string(),
                ));
            }
            check_function(func, file, diagnostics, in_class);
            check_stmts(&func.body, file, diagnostics, false, level);
            // Check decorator expressions
            for decorator in &func.decorator_list {
                check_expr(&decorator.expression, file, diagnostics, level);
            }
        }
        Stmt::ClassDef(cls) => {
            if level < Level::Types {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CLASS,
                    file,
                    cls.name.range(),
                    "`class` is not allowed at this level".to_string(),
                ));
            } else {
                // Check for methods (FunctionDef inside class) before level 4
                if level < Level::Classes {
                    for body_stmt in &cls.body {
                        if let Stmt::FunctionDef(func) = body_stmt {
                            diagnostics.push(make_lint_diagnostic(
                                &FORBIDDEN_CLASS_METHOD,
                                file,
                                func.name.range(),
                                "Methods in classes are not allowed at this level".to_string(),
                            ));
                        }
                    }
                }
                // Skip annotation check for Enum subclasses — Enum members
                // don't need (and shouldn't have) type annotations.
                let is_enum = cls.arguments.as_ref().is_some_and(|args| {
                    args.args
                        .iter()
                        .any(|arg| matches!(arg, Expr::Name(name) if name.id == "Enum"))
                });
                if !is_enum {
                    check_class_body(cls, file, diagnostics);
                }
            }
            check_stmts(&cls.body, file, diagnostics, true, level);
            // Check decorator expressions
            for decorator in &cls.decorator_list {
                check_expr(&decorator.expression, file, diagnostics, level);
            }
        }
        Stmt::For(for_stmt) => {
            if for_stmt.is_async && level < Level::Full {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CONSTRUCT,
                    file,
                    for_stmt.range(),
                    "`async for` is not allowed".to_string(),
                ));
            } else if level < Level::Arrays {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_LOOP,
                    file,
                    for_stmt.range(),
                    "`for` loop is not allowed at this level".to_string(),
                ));
            }
            check_expr(&for_stmt.iter, file, diagnostics, level);
            check_stmts(&for_stmt.body, file, diagnostics, in_class, level);
            check_stmts(&for_stmt.orelse, file, diagnostics, in_class, level);
        }
        Stmt::While(while_stmt) => {
            if level < Level::Arrays {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_LOOP,
                    file,
                    while_stmt.range(),
                    "`while` loop is not allowed at this level".to_string(),
                ));
            }
            check_expr(&while_stmt.test, file, diagnostics, level);
            check_stmts(&while_stmt.body, file, diagnostics, in_class, level);
            check_stmts(&while_stmt.orelse, file, diagnostics, in_class, level);
        }
        Stmt::If(if_stmt) => {
            if level < Level::Selection {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_SELECTION,
                    file,
                    if_stmt.range(),
                    "`if` is not allowed at this level".to_string(),
                ));
            }
            check_expr(&if_stmt.test, file, diagnostics, level);
            check_stmts(&if_stmt.body, file, diagnostics, in_class, level);
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = &clause.test {
                    check_expr(test, file, diagnostics, level);
                }
                check_stmts(&clause.body, file, diagnostics, in_class, level);
            }
        }
        Stmt::Match(match_stmt) => {
            if level < Level::Types {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_MATCH,
                    file,
                    match_stmt.range(),
                    "`match` is not allowed at this level".to_string(),
                ));
            }
            check_expr(&match_stmt.subject, file, diagnostics, level);
            for case in &match_stmt.cases {
                if let Some(guard) = &case.guard {
                    check_expr(guard, file, diagnostics, level);
                }
                check_stmts(&case.body, file, diagnostics, in_class, level);
            }
        }
        Stmt::With(with_stmt) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                with_stmt.range(),
                "`with` is not allowed".to_string(),
            ));
        }
        Stmt::Try(try_stmt) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                try_stmt.range(),
                "`try` is not allowed".to_string(),
            ));
        }
        Stmt::Global(global_stmt) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                global_stmt.range(),
                "`global` is not allowed".to_string(),
            ));
        }
        Stmt::Nonlocal(nonlocal_stmt) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                nonlocal_stmt.range(),
                "`nonlocal` is not allowed".to_string(),
            ));
        }
        Stmt::Delete(del_stmt) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                del_stmt.range(),
                "`del` is not allowed".to_string(),
            ));
        }
        Stmt::AugAssign(aug) => {
            if level < Level::Arrays {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_AUG_ASSIGN,
                    file,
                    aug.range(),
                    "Augmented assignment is not allowed at this level".to_string(),
                ));
            }
            check_expr(&aug.value, file, diagnostics, level);
        }
        // Statements that are always allowed but may contain expressions to check
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                check_expr(value, file, diagnostics, level);
            }
        }
        Stmt::Assign(assign) => {
            check_expr(&assign.value, file, diagnostics, level);
        }
        Stmt::AnnAssign(ann) => {
            if let Some(value) = &ann.value {
                check_expr(value, file, diagnostics, level);
            }
        }
        Stmt::Expr(expr_stmt) => {
            check_expr(&expr_stmt.value, file, diagnostics, level);
        }
        Stmt::Assert(assert_stmt) => {
            check_expr(&assert_stmt.test, file, diagnostics, level);
            if let Some(msg) = &assert_stmt.msg {
                check_expr(msg, file, diagnostics, level);
            }
        }
        Stmt::Raise(raise) => {
            if let Some(exc) = &raise.exc {
                check_expr(exc, file, diagnostics, level);
            }
        }
        // Always allowed (or allowed at level 5), no sub-expressions to check
        Stmt::Import(_)
        | Stmt::ImportFrom(_)
        | Stmt::Pass(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::TypeAlias(_)
        | Stmt::IpyEscapeCommand(_)
        | Stmt::With(_)
        | Stmt::Try(_)
        | Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Delete(_) => {}
    }
}

/// Recursively check an expression for forbidden constructs.
fn check_expr(expr: &Expr, file: File, diagnostics: &mut Vec<Diagnostic>, level: Level) {
    match expr {
        // Collection literals
        Expr::List(list) if level < Level::Arrays => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                list.range(),
                "List literal is not allowed at this level".to_string(),
            ));
        }
        Expr::Tuple(tuple) if level < Level::Arrays => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                tuple.range(),
                "Tuple literal is not allowed at this level".to_string(),
            ));
        }
        Expr::Dict(dict) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                dict.range(),
                "Dict literal is not allowed at this level".to_string(),
            ));
        }
        Expr::Set(set) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                set.range(),
                "Set literal is not allowed at this level".to_string(),
            ));
        }

        // Comprehensions
        Expr::ListComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                "List comprehension is not allowed at this level".to_string(),
            ));
        }
        Expr::SetComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                "Set comprehension is not allowed at this level".to_string(),
            ));
        }
        Expr::DictComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                "Dict comprehension is not allowed at this level".to_string(),
            ));
        }
        Expr::Generator(generator) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                generator.range(),
                "Generator expression is not allowed at this level".to_string(),
            ));
        }

        // Lambda
        Expr::Lambda(lambda) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_LAMBDA,
                file,
                lambda.range(),
                "`lambda` is not allowed at this level".to_string(),
            ));
        }

        // Yield / await (forbidden below level 5)
        Expr::Yield(y) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                y.range(),
                "`yield` is not allowed".to_string(),
            ));
        }
        Expr::YieldFrom(y) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                y.range(),
                "`yield from` is not allowed".to_string(),
            ));
        }
        Expr::Await(a) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                a.range(),
                "`await` is not allowed".to_string(),
            ));
        }

        // For all other expressions, just recurse into children below.
        _ => {}
    }

    // Recurse into sub-expressions.
    match expr {
        Expr::BoolOp(e) => {
            for v in &e.values {
                check_expr(v, file, diagnostics, level);
            }
        }
        Expr::Named(e) => {
            check_expr(&e.target, file, diagnostics, level);
            check_expr(&e.value, file, diagnostics, level);
        }
        Expr::BinOp(e) => {
            check_expr(&e.left, file, diagnostics, level);
            check_expr(&e.right, file, diagnostics, level);
        }
        Expr::UnaryOp(e) => check_expr(&e.operand, file, diagnostics, level),
        Expr::Lambda(e) => check_expr(&e.body, file, diagnostics, level),
        Expr::If(e) => {
            check_expr(&e.test, file, diagnostics, level);
            check_expr(&e.body, file, diagnostics, level);
            check_expr(&e.orelse, file, diagnostics, level);
        }
        Expr::Dict(e) => {
            for item in &e.items {
                if let Some(k) = &item.key {
                    check_expr(k, file, diagnostics, level);
                }
                check_expr(&item.value, file, diagnostics, level);
            }
        }
        Expr::Set(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level);
            }
        }
        Expr::ListComp(e) => check_expr(&e.elt, file, diagnostics, level),
        Expr::SetComp(e) => check_expr(&e.elt, file, diagnostics, level),
        Expr::DictComp(e) => {
            check_expr(&e.key, file, diagnostics, level);
            check_expr(&e.value, file, diagnostics, level);
        }
        Expr::Generator(generator) => check_expr(&generator.elt, file, diagnostics, level),
        Expr::Await(e) => check_expr(&e.value, file, diagnostics, level),
        Expr::Yield(e) => {
            if let Some(v) = &e.value {
                check_expr(v, file, diagnostics, level);
            }
        }
        Expr::YieldFrom(e) => check_expr(&e.value, file, diagnostics, level),
        Expr::Compare(e) => {
            check_expr(&e.left, file, diagnostics, level);
            for c in &e.comparators {
                check_expr(c, file, diagnostics, level);
            }
        }
        Expr::Call(e) => {
            check_expr(&e.func, file, diagnostics, level);
            for arg in &e.arguments.args {
                check_expr(arg, file, diagnostics, level);
            }
            for kw in &e.arguments.keywords {
                check_expr(&kw.value, file, diagnostics, level);
            }
        }
        Expr::Attribute(e) => check_expr(&e.value, file, diagnostics, level),
        Expr::Subscript(e) => {
            check_expr(&e.value, file, diagnostics, level);
            check_expr(&e.slice, file, diagnostics, level);
        }
        Expr::Starred(e) => check_expr(&e.value, file, diagnostics, level),
        Expr::List(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level);
            }
        }
        Expr::Tuple(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level);
            }
        }
        Expr::Slice(e) => {
            if let Some(v) = &e.lower {
                check_expr(v, file, diagnostics, level);
            }
            if let Some(v) = &e.upper {
                check_expr(v, file, diagnostics, level);
            }
            if let Some(v) = &e.step {
                check_expr(v, file, diagnostics, level);
            }
        }
        // Leaf expressions: no children to recurse into
        Expr::Name(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::FString(_)
        | Expr::TString(_)
        | Expr::IpyEscapeCommand(_) => {}
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
    diag.annotate(Annotation::primary(Span::from(file).with_range(range)));
    diag
}
