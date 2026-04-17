//! Annotation checker and construct restriction checker for spython.
//!
//! This module checks for missing annotations and forbidden constructs based
//! on the current teaching level.

use ruff_db::diagnostic::{Annotation, Diagnostic, DiagnosticId, Severity, Span};
use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_python_ast::{
    Expr, Operator, Parameter, Stmt, StmtAssign, StmtClassDef, StmtFunctionDef, UnaryOp,
};
use ruff_text_size::Ranged;
use ty_project::Db;
use ty_python_semantic::types::KnownClass;
use ty_python_semantic::{HasType, SemanticModel};

use crate::lints::{
    BARE_EXPRESSION, BOOL_IN_ARITHMETIC, CHAINED_COMPARISON, FORBIDDEN_AUG_ASSIGN, FORBIDDEN_CLASS,
    FORBIDDEN_CLASS_METHOD, FORBIDDEN_COLLECTION_LITERAL, FORBIDDEN_COMPREHENSION,
    FORBIDDEN_CONSTRUCT, FORBIDDEN_DEFAULT_ARG, FORBIDDEN_LAMBDA, FORBIDDEN_LOOP, FORBIDDEN_MATCH,
    FORBIDDEN_SELECTION, MISSING_ATTRIBUTE_ANNOTATION, MISSING_PARAMETER_ANNOTATION,
    MISSING_RETURN_ANNOTATION, NON_BOOLEAN_CONDITION,
};

/// Teaching level that controls which Python constructs are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// def, return, scalars (int, float, str, bool), string indexing.
    Functions = 0,
    /// Adds: if/elif/else.
    Selection = 1,
    /// Adds: Enum, @dataclass, match.
    UserTypes = 2,
    /// Adds: list literals, for, while, augmented assignment.
    Repetition = 3,
    /// Adds: full classes with methods, dict/set literals, comprehensions, lambda.
    Classes = 4,
    /// Unrestricted Python (only annotations still required).
    Full = 5,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (n, name) = match self {
            Level::Functions => (0, "Functions"),
            Level::Selection => (1, "Selection"),
            Level::UserTypes => (2, "User types"),
            Level::Repetition => (3, "Repetition"),
            Level::Classes => (4, "Classes"),
            Level::Full => (5, "Full"),
        };
        write!(f, "{n} - {name}")
    }
}

impl Level {
    pub fn from_u8(n: u8) -> Option<Level> {
        match n {
            0 => Some(Level::Functions),
            1 => Some(Level::Selection),
            2 => Some(Level::UserTypes),
            3 => Some(Level::Repetition),
            4 => Some(Level::Classes),
            5 => Some(Level::Full),
            _ => None,
        }
    }
}

/// Check a file for missing annotations and forbidden constructs.
///
/// `in_repl` is true when the check is running against REPL input; a bare
/// expression statement is valid in a REPL (its value is displayed) so the
/// `BARE_EXPRESSION` lint is suppressed in that context.
pub fn check_file_annotations(
    db: &dyn Db,
    file: File,
    level: Level,
    in_repl: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let parsed = parsed_module(db, file);
    let module = parsed.load(db);
    let model = SemanticModel::new(db, file);
    let checker = Checker {
        file,
        model: &model,
        level,
        in_repl,
    };
    checker.check_stmts(module.suite(), &mut diagnostics, false, false);
    diagnostics
}

/// Per-file immutable context shared by every recursive check call.
///
/// `in_class` and `in_doctest` flip as recursion enters a class body or a
/// synthetic doctest wrapper function, so they stay as method parameters
/// instead of fields.
struct Checker<'a> {
    file: File,
    model: &'a SemanticModel<'a>,
    level: Level,
    in_repl: bool,
}

impl Checker<'_> {
    /// Recursively check a list of statements.
    ///
    /// `in_doctest` is true when we are inside a synthetic
    /// `__spython_doctest_N__` wrapper function — it suppresses
    /// `BARE_EXPRESSION` the same way `in_repl` does, since `>>> x` in a
    /// docstring is a value-display idiom.
    fn check_stmts(
        &self,
        stmts: &[Stmt],
        diagnostics: &mut Vec<Diagnostic>,
        in_class: bool,
        in_doctest: bool,
    ) {
        for stmt in stmts {
            self.check_stmt(stmt, diagnostics, in_class, in_doctest);
        }
    }

    /// Check a single statement for annotations and forbidden constructs.
    fn check_stmt(
        &self,
        stmt: &Stmt,
        diagnostics: &mut Vec<Diagnostic>,
        in_class: bool,
        in_doctest: bool,
    ) {
        let file = self.file;
        let level = self.level;
        let model = self.model;
        match stmt {
            Stmt::FunctionDef(func) => {
                if func.is_async && level < Level::Full {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_CONSTRUCT,
                        file,
                        func.range(),
                        forbidden_msg("`async def`", level, Level::Full),
                    ));
                }
                check_function(func, file, diagnostics, in_class, level);
                let body_in_doctest =
                    in_doctest || crate::doctests::is_synthetic_fn_name(func.name.as_str());
                self.check_stmts(&func.body, diagnostics, false, body_in_doctest);
                check_decorators(&func.decorator_list, file, diagnostics, level, model);
            }
            Stmt::ClassDef(cls) => {
                if level < Level::UserTypes {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_CLASS,
                        file,
                        cls.name.range(),
                        forbidden_msg("`class`", level, Level::UserTypes),
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
                                    forbidden_msg("Methods in classes", level, Level::Classes),
                                ));
                            }
                        }
                    }
                    // Skip annotation check for Enum subclasses — Enum members
                    // don't need (and shouldn't have) type annotations.
                    let is_enum = cls.arguments.as_ref().is_some_and(|args| {
                        args.args.iter().any(|arg| {
                            matches!(arg, Expr::Name(name)
                                if matches!(name.id.as_str(),
                                    "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag"))
                        })
                    });
                    if !is_enum {
                        check_class_body(cls, file, diagnostics);
                    }
                }
                self.check_stmts(&cls.body, diagnostics, true, in_doctest);
                check_decorators(&cls.decorator_list, file, diagnostics, level, model);
            }
            Stmt::For(for_stmt) => {
                if for_stmt.is_async && level < Level::Full {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_CONSTRUCT,
                        file,
                        for_stmt.range(),
                        forbidden_msg("`async for`", level, Level::Full),
                    ));
                } else if level < Level::Repetition {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_LOOP,
                        file,
                        for_stmt.range(),
                        forbidden_msg("`for` loop", level, Level::Repetition),
                    ));
                }
                check_expr(&for_stmt.iter, file, diagnostics, level, model);
                self.check_stmts(&for_stmt.body, diagnostics, in_class, in_doctest);
                self.check_stmts(&for_stmt.orelse, diagnostics, in_class, in_doctest);
            }
            Stmt::While(while_stmt) => {
                if level < Level::Repetition {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_LOOP,
                        file,
                        while_stmt.range(),
                        forbidden_msg("`while` loop", level, Level::Repetition),
                    ));
                }
                check_bool_ctx(&while_stmt.test, file, diagnostics, level, model);
                check_expr(&while_stmt.test, file, diagnostics, level, model);
                self.check_stmts(&while_stmt.body, diagnostics, in_class, in_doctest);
                self.check_stmts(&while_stmt.orelse, diagnostics, in_class, in_doctest);
            }
            Stmt::If(if_stmt) => {
                if level < Level::Selection {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_SELECTION,
                        file,
                        if_stmt.range(),
                        forbidden_msg("`if`", level, Level::Selection),
                    ));
                }
                check_bool_ctx(&if_stmt.test, file, diagnostics, level, model);
                check_expr(&if_stmt.test, file, diagnostics, level, model);
                self.check_stmts(&if_stmt.body, diagnostics, in_class, in_doctest);
                for clause in &if_stmt.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        check_bool_ctx(test, file, diagnostics, level, model);
                        check_expr(test, file, diagnostics, level, model);
                    }
                    self.check_stmts(&clause.body, diagnostics, in_class, in_doctest);
                }
            }
            Stmt::Match(match_stmt) => {
                if level < Level::UserTypes {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_MATCH,
                        file,
                        match_stmt.range(),
                        forbidden_msg("`match`", level, Level::UserTypes),
                    ));
                }
                check_expr(&match_stmt.subject, file, diagnostics, level, model);
                for case in &match_stmt.cases {
                    if let Some(guard) = &case.guard {
                        check_expr(guard, file, diagnostics, level, model);
                    }
                    self.check_stmts(&case.body, diagnostics, in_class, in_doctest);
                }
            }
            Stmt::With(with_stmt) => {
                if level < Level::Full {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_CONSTRUCT,
                        file,
                        with_stmt.range(),
                        forbidden_msg("`with`", level, Level::Full),
                    ));
                }
                for item in &with_stmt.items {
                    check_expr(&item.context_expr, file, diagnostics, level, model);
                }
                self.check_stmts(&with_stmt.body, diagnostics, in_class, in_doctest);
            }
            Stmt::Try(try_stmt) => {
                if level < Level::Full {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_CONSTRUCT,
                        file,
                        try_stmt.range(),
                        forbidden_msg("`try`", level, Level::Full),
                    ));
                }
                self.check_stmts(&try_stmt.body, diagnostics, in_class, in_doctest);
                for handler in &try_stmt.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.check_stmts(&h.body, diagnostics, in_class, in_doctest);
                }
                self.check_stmts(&try_stmt.orelse, diagnostics, in_class, in_doctest);
                self.check_stmts(&try_stmt.finalbody, diagnostics, in_class, in_doctest);
            }
            Stmt::Global(global_stmt) if level < Level::Full => {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CONSTRUCT,
                    file,
                    global_stmt.range(),
                    forbidden_msg("`global`", level, Level::Full),
                ));
            }
            Stmt::Nonlocal(nonlocal_stmt) if level < Level::Full => {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CONSTRUCT,
                    file,
                    nonlocal_stmt.range(),
                    forbidden_msg("`nonlocal`", level, Level::Full),
                ));
            }
            Stmt::Delete(del_stmt) if level < Level::Full => {
                diagnostics.push(make_lint_diagnostic(
                    &FORBIDDEN_CONSTRUCT,
                    file,
                    del_stmt.range(),
                    forbidden_msg("`del`", level, Level::Full),
                ));
            }
            Stmt::AugAssign(aug) => {
                if level < Level::Repetition {
                    diagnostics.push(make_lint_diagnostic(
                        &FORBIDDEN_AUG_ASSIGN,
                        file,
                        aug.range(),
                        forbidden_msg("Augmented assignment", level, Level::Repetition),
                    ));
                }
                if is_arithmetic_op(aug.op) {
                    reject_bool_arith_operand(&aug.target, file, diagnostics, level, model);
                    reject_bool_arith_operand(&aug.value, file, diagnostics, level, model);
                }
                check_expr(&aug.value, file, diagnostics, level, model);
            }
            // Statements that are always allowed but may contain expressions to check
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    check_expr(value, file, diagnostics, level, model);
                }
            }
            Stmt::Assign(assign) => {
                check_expr(&assign.value, file, diagnostics, level, model);
            }
            Stmt::AnnAssign(ann) => {
                if let Some(value) = &ann.value {
                    check_expr(value, file, diagnostics, level, model);
                }
            }
            Stmt::Expr(expr_stmt) => {
                if !self.in_repl
                    && !in_doctest
                    && level < Level::Classes
                    && !matches!(
                        &*expr_stmt.value,
                        Expr::Call(_) | Expr::StringLiteral(_) | Expr::EllipsisLiteral(_)
                    )
                {
                    diagnostics.push(make_lint_diagnostic(
                        &BARE_EXPRESSION,
                        file,
                        expr_stmt.range(),
                        "Expression statement has no effect; did you forget `=` or `print(...)`?"
                            .to_owned(),
                    ));
                }
                check_expr(&expr_stmt.value, file, diagnostics, level, model);
            }
            Stmt::Assert(assert_stmt) => {
                check_bool_ctx(&assert_stmt.test, file, diagnostics, level, model);
                check_expr(&assert_stmt.test, file, diagnostics, level, model);
                if let Some(msg) = &assert_stmt.msg {
                    check_expr(msg, file, diagnostics, level, model);
                }
            }
            Stmt::Raise(raise) => {
                if let Some(exc) = &raise.exc {
                    check_expr(exc, file, diagnostics, level, model);
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
            | Stmt::Global(_)
            | Stmt::Nonlocal(_)
            | Stmt::Delete(_) => {}
        }
    }
}

/// Returns true if `op` is an arithmetic operator (`+`, `-`, `*`, `/`,
/// `//`, `%`, `**`). Bitwise and matrix operators are excluded.
fn is_arithmetic_op(op: Operator) -> bool {
    matches!(
        op,
        Operator::Add
            | Operator::Sub
            | Operator::Mult
            | Operator::Div
            | Operator::FloorDiv
            | Operator::Mod
            | Operator::Pow
    )
}

/// Reject an arithmetic operand whose inferred type is `bool`. Only applies
/// at teaching levels 0–3.
fn reject_bool_arith_operand(
    expr: &Expr,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    level: Level,
    model: &SemanticModel<'_>,
) {
    if level >= Level::Classes {
        return;
    }
    let Some(ty) = expr.inferred_type(model) else {
        return;
    };
    let bool_ty = KnownClass::Bool.to_instance(model.db());
    if ty.is_assignable_to(model.db(), bool_ty) {
        diagnostics.push(make_lint_diagnostic(
            &BOOL_IN_ARITHMETIC,
            file,
            expr.range(),
            format!(
                "Operand has type `{}`; `bool` is not allowed in arithmetic expressions",
                ty.display(model.db())
            ),
        ));
    }
}

/// Check that `expr` (used in a boolean context) has type `bool`.
///
/// Skips `and`/`or`/`not` — their operands are checked individually as they
/// are each in a boolean context, so reporting on the compound expression
/// would be redundant. Only applies at levels 0–3.
fn check_bool_ctx(
    expr: &Expr,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    level: Level,
    model: &SemanticModel<'_>,
) {
    if level >= Level::Classes {
        return;
    }
    if matches!(expr, Expr::BoolOp(_)) {
        return;
    }
    if let Expr::UnaryOp(u) = expr
        && u.op == UnaryOp::Not
    {
        return;
    }
    let Some(ty) = expr.inferred_type(model) else {
        return;
    };
    let bool_ty = KnownClass::Bool.to_instance(model.db());
    if !ty.is_assignable_to(model.db(), bool_ty) {
        diagnostics.push(make_lint_diagnostic(
            &NON_BOOLEAN_CONDITION,
            file,
            expr.range(),
            format!(
                "Condition has type `{}`, expected `bool`",
                ty.display(model.db())
            ),
        ));
    }
}

/// Recursively check an expression for forbidden constructs.
fn check_expr(
    expr: &Expr,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    level: Level,
    model: &SemanticModel<'_>,
) {
    match expr {
        // Collection literals
        Expr::List(list) if level < Level::Repetition => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                list.range(),
                forbidden_msg("List literal", level, Level::Repetition),
            ));
        }
        Expr::Tuple(tuple) if level < Level::Repetition => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                tuple.range(),
                forbidden_msg("Tuple literal", level, Level::Repetition),
            ));
        }
        Expr::Dict(dict) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                dict.range(),
                forbidden_msg("Dict literal", level, Level::Classes),
            ));
        }
        Expr::Set(set) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COLLECTION_LITERAL,
                file,
                set.range(),
                forbidden_msg("Set literal", level, Level::Classes),
            ));
        }

        // Comprehensions
        Expr::ListComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                forbidden_msg("List comprehension", level, Level::Classes),
            ));
        }
        Expr::SetComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                forbidden_msg("Set comprehension", level, Level::Classes),
            ));
        }
        Expr::DictComp(comp) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                comp.range(),
                forbidden_msg("Dict comprehension", level, Level::Classes),
            ));
        }
        Expr::Generator(generator) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_COMPREHENSION,
                file,
                generator.range(),
                forbidden_msg("Generator expression", level, Level::Classes),
            ));
        }

        // Lambda
        Expr::Lambda(lambda) if level < Level::Classes => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_LAMBDA,
                file,
                lambda.range(),
                forbidden_msg("`lambda`", level, Level::Classes),
            ));
        }

        // Chained comparison (a < b < c, a == b != c, …)
        Expr::Compare(cmp) if level < Level::Classes && cmp.comparators.len() > 1 => {
            diagnostics.push(make_lint_diagnostic(
                &CHAINED_COMPARISON,
                file,
                cmp.range(),
                "Chained comparison is not allowed; split with `and` / `or`".to_owned(),
            ));
        }

        // Yield / await (forbidden below level 5)
        Expr::Yield(y) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                y.range(),
                forbidden_msg("`yield`", level, Level::Full),
            ));
        }
        Expr::YieldFrom(y) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                y.range(),
                forbidden_msg("`yield from`", level, Level::Full),
            ));
        }
        Expr::Await(a) if level < Level::Full => {
            diagnostics.push(make_lint_diagnostic(
                &FORBIDDEN_CONSTRUCT,
                file,
                a.range(),
                forbidden_msg("`await`", level, Level::Full),
            ));
        }

        // For all other expressions, just recurse into children below.
        _ => {}
    }

    // Recurse into sub-expressions.
    match expr {
        Expr::BoolOp(e) => {
            for v in &e.values {
                check_bool_ctx(v, file, diagnostics, level, model);
                check_expr(v, file, diagnostics, level, model);
            }
        }
        Expr::Named(e) => {
            check_expr(&e.target, file, diagnostics, level, model);
            check_expr(&e.value, file, diagnostics, level, model);
        }
        Expr::BinOp(e) => {
            if is_arithmetic_op(e.op) {
                reject_bool_arith_operand(&e.left, file, diagnostics, level, model);
                reject_bool_arith_operand(&e.right, file, diagnostics, level, model);
            }
            check_expr(&e.left, file, diagnostics, level, model);
            check_expr(&e.right, file, diagnostics, level, model);
        }
        Expr::UnaryOp(e) => {
            match e.op {
                UnaryOp::Not => check_bool_ctx(&e.operand, file, diagnostics, level, model),
                UnaryOp::UAdd | UnaryOp::USub => {
                    reject_bool_arith_operand(&e.operand, file, diagnostics, level, model)
                }
                UnaryOp::Invert => {}
            }
            check_expr(&e.operand, file, diagnostics, level, model);
        }
        Expr::Lambda(e) => check_expr(&e.body, file, diagnostics, level, model),
        Expr::If(e) => {
            check_bool_ctx(&e.test, file, diagnostics, level, model);
            check_expr(&e.test, file, diagnostics, level, model);
            check_expr(&e.body, file, diagnostics, level, model);
            check_expr(&e.orelse, file, diagnostics, level, model);
        }
        Expr::Dict(e) => {
            for item in &e.items {
                if let Some(k) = &item.key {
                    check_expr(k, file, diagnostics, level, model);
                }
                check_expr(&item.value, file, diagnostics, level, model);
            }
        }
        Expr::Set(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level, model);
            }
        }
        Expr::ListComp(e) => check_expr(&e.elt, file, diagnostics, level, model),
        Expr::SetComp(e) => check_expr(&e.elt, file, diagnostics, level, model),
        Expr::DictComp(e) => {
            check_expr(&e.key, file, diagnostics, level, model);
            check_expr(&e.value, file, diagnostics, level, model);
        }
        Expr::Generator(generator) => check_expr(&generator.elt, file, diagnostics, level, model),
        Expr::Await(e) => check_expr(&e.value, file, diagnostics, level, model),
        Expr::Yield(e) => {
            if let Some(v) = &e.value {
                check_expr(v, file, diagnostics, level, model);
            }
        }
        Expr::YieldFrom(e) => check_expr(&e.value, file, diagnostics, level, model),
        Expr::Compare(e) => {
            check_expr(&e.left, file, diagnostics, level, model);
            for c in &e.comparators {
                check_expr(c, file, diagnostics, level, model);
            }
        }
        Expr::Call(e) => {
            check_expr(&e.func, file, diagnostics, level, model);
            for arg in &e.arguments.args {
                check_expr(arg, file, diagnostics, level, model);
            }
            for kw in &e.arguments.keywords {
                check_expr(&kw.value, file, diagnostics, level, model);
            }
        }
        Expr::Attribute(e) => check_expr(&e.value, file, diagnostics, level, model),
        Expr::Subscript(e) => {
            check_expr(&e.value, file, diagnostics, level, model);
            check_expr(&e.slice, file, diagnostics, level, model);
        }
        Expr::Starred(e) => check_expr(&e.value, file, diagnostics, level, model),
        Expr::List(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level, model);
            }
        }
        Expr::Tuple(e) => {
            for v in &e.elts {
                check_expr(v, file, diagnostics, level, model);
            }
        }
        Expr::Slice(e) => {
            if let Some(v) = &e.lower {
                check_expr(v, file, diagnostics, level, model);
            }
            if let Some(v) = &e.upper {
                check_expr(v, file, diagnostics, level, model);
            }
            if let Some(v) = &e.step {
                check_expr(v, file, diagnostics, level, model);
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

fn check_decorators(
    decorator_list: &[ruff_python_ast::Decorator],
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    level: Level,
    model: &SemanticModel<'_>,
) {
    for decorator in decorator_list {
        check_expr(&decorator.expression, file, diagnostics, level, model);
    }
}

fn check_param_default(
    pwd: &ruff_python_ast::ParameterWithDefault,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    level: Level,
) {
    if level >= Level::Classes {
        return;
    }
    if let Some(default) = &pwd.default {
        diagnostics.push(make_lint_diagnostic(
            &FORBIDDEN_DEFAULT_ARG,
            file,
            default.range(),
            format!(
                "Default value for parameter `{}` is not allowed at level {level}, requires level {}",
                pwd.parameter.name.as_str(),
                Level::Classes,
            ),
        ));
    }
}

fn check_param_annotation(
    param: &Parameter,
    prefix: &str,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if param.annotation.is_none() {
        diagnostics.push(make_lint_diagnostic(
            &MISSING_PARAMETER_ANNOTATION,
            file,
            param.range(),
            format!(
                "Parameter `{prefix}{}` is missing a type annotation",
                param.name.as_str()
            ),
        ));
    }
}

/// Check a function definition for missing parameter and return annotations.
fn check_function(
    func: &StmtFunctionDef,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
    in_class: bool,
    level: Level,
) {
    let params = &func.parameters;

    // Skip the first positional parameter in a method (the implicit receiver: self/cls),
    // unless the method is decorated with @staticmethod.
    let is_static = func
        .decorator_list
        .iter()
        .any(|d| matches!(&d.expression, Expr::Name(name) if name.id.as_str() == "staticmethod"));
    let mut skip_first = in_class && !is_static;

    for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
        if skip_first {
            skip_first = false;
            continue;
        }
        check_param_annotation(&pwd.parameter, "", file, diagnostics);
        check_param_default(pwd, file, diagnostics, level);
    }

    for pwd in &params.kwonlyargs {
        check_param_annotation(&pwd.parameter, "", file, diagnostics);
        check_param_default(pwd, file, diagnostics, level);
    }

    if let Some(vararg) = &params.vararg {
        check_param_annotation(vararg, "*", file, diagnostics);
    }

    if let Some(kwarg) = &params.kwarg {
        check_param_annotation(kwarg, "**", file, diagnostics);
    }

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

/// Format a "not allowed" message with current and required levels.
fn forbidden_msg(construct: &str, level: Level, min_level: Level) -> String {
    format!("{construct} is not allowed at level {level}, requires level {min_level}")
}

/// Create a lint diagnostic that matches ty's format.
pub(crate) fn make_lint_diagnostic(
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
