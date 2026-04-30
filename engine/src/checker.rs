//! Annotation checker and construct restriction checker for spython.
//!
//! This module checks for missing annotations and forbidden constructs based
//! on the current teaching level.

use ruff_db::diagnostic::{Annotation, Diagnostic, DiagnosticId, Severity, Span};
use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use std::collections::HashSet;

use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{
    CmpOp, Decorator, Expr, ExprContext, Operator, Parameter, Stmt, StmtAssign, StmtClassDef,
    StmtFunctionDef, UnaryOp,
};
use ruff_python_stdlib::str as stdlib_str;
use ruff_text_size::{Ranged, TextRange, TextSize};
use ty_project::Db;
use ty_python_semantic::types::KnownClass;
use ty_python_semantic::{HasType, SemanticModel};

use crate::lints::{
    BARE_EXPRESSION, BOOL_IN_ARITHMETIC, CHAINED_COMPARISON, F_STRING_MISSING_PLACEHOLDERS,
    FORBIDDEN_AUG_ASSIGN, FORBIDDEN_CLASS, FORBIDDEN_CLASS_METHOD, FORBIDDEN_COLLECTION_LITERAL,
    FORBIDDEN_COMPREHENSION, FORBIDDEN_CONSTRUCT, FORBIDDEN_DEFAULT_ARG, FORBIDDEN_LAMBDA,
    FORBIDDEN_LOOP, FORBIDDEN_MATCH, FORBIDDEN_SELECTION, INVALID_ARGUMENT_NAME,
    INVALID_CLASS_NAME, INVALID_FIRST_ARGUMENT_NAME_FOR_METHOD, INVALID_FUNCTION_NAME,
    MISSING_ATTRIBUTE_ANNOTATION, MISSING_PARAMETER_ANNOTATION, MISSING_RETURN_ANNOTATION,
    NEEDLESS_BOOL, NON_BOOLEAN_CONDITION, NON_LOWERCASE_VARIABLE_IN_FUNCTION,
    NON_UPPERCASE_ENUM_MEMBER, NONE_COMPARISON, NOT_IN_TEST, NOT_IS_TEST, TRUE_FALSE_COMPARISON,
    UNNECESSARY_PASS, UNUSED_VARIABLE,
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
    checker.check_stmts(module.suite(), &mut diagnostics, false, false, false);
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
        in_function: bool,
        in_doctest: bool,
    ) {
        if stmts.len() > 1 {
            for stmt in stmts {
                if matches!(stmt, Stmt::Pass(_)) {
                    diagnostics.push(make_lint_diagnostic(
                        &UNNECESSARY_PASS,
                        self.file,
                        stmt.range(),
                        "Unnecessary `pass` statement".to_owned(),
                    ));
                }
            }
        }
        for stmt in stmts {
            self.check_stmt(stmt, diagnostics, in_class, in_function, in_doctest);
        }
    }

    /// Check a single statement for annotations and forbidden constructs.
    fn check_stmt(
        &self,
        stmt: &Stmt,
        diagnostics: &mut Vec<Diagnostic>,
        in_class: bool,
        in_function: bool,
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
                let synthetic = crate::doctests::is_synthetic_fn_name(func.name.as_str());
                if !synthetic {
                    check_function_name(func, file, diagnostics);
                }
                if level <= Level::Repetition && !synthetic {
                    check_unused_variables(&func.body, file, diagnostics);
                }
                let body_in_doctest = in_doctest || synthetic;
                self.check_stmts(&func.body, diagnostics, false, true, body_in_doctest);
                check_decorators(&func.decorator_list, file, diagnostics, level, model);
            }
            Stmt::ClassDef(cls) => {
                check_class_name(cls, file, diagnostics);
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
                    if is_enum_class(cls) {
                        check_enum_member_names(cls, file, diagnostics);
                    } else {
                        check_class_body(cls, file, diagnostics);
                    }
                }
                self.check_stmts(&cls.body, diagnostics, true, false, in_doctest);
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
                if in_function {
                    check_assign_target_names(&for_stmt.target, file, diagnostics);
                }
                self.check_stmts(
                    &for_stmt.body,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
                self.check_stmts(
                    &for_stmt.orelse,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
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
                self.check_stmts(
                    &while_stmt.body,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
                self.check_stmts(
                    &while_stmt.orelse,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
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
                check_needless_bool(if_stmt, file, diagnostics);
                check_bool_ctx(&if_stmt.test, file, diagnostics, level, model);
                check_expr(&if_stmt.test, file, diagnostics, level, model);
                self.check_stmts(
                    &if_stmt.body,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
                for clause in &if_stmt.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        check_bool_ctx(test, file, diagnostics, level, model);
                        check_expr(test, file, diagnostics, level, model);
                    }
                    self.check_stmts(&clause.body, diagnostics, in_class, in_function, in_doctest);
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
                    self.check_stmts(&case.body, diagnostics, in_class, in_function, in_doctest);
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
                self.check_stmts(
                    &with_stmt.body,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
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
                self.check_stmts(
                    &try_stmt.body,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
                for handler in &try_stmt.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.check_stmts(&h.body, diagnostics, in_class, in_function, in_doctest);
                }
                self.check_stmts(
                    &try_stmt.orelse,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
                self.check_stmts(
                    &try_stmt.finalbody,
                    diagnostics,
                    in_class,
                    in_function,
                    in_doctest,
                );
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
                if in_function {
                    for target in &assign.targets {
                        check_assign_target_names(target, file, diagnostics);
                    }
                }
                check_expr(&assign.value, file, diagnostics, level, model);
            }
            Stmt::AnnAssign(ann) => {
                if in_function {
                    check_assign_target_names(&ann.target, file, diagnostics);
                }
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
        Expr::Compare(cmp) => {
            check_literal_comparisons(cmp, file, diagnostics);
        }
        Expr::UnaryOp(u) if u.op == UnaryOp::Not => {
            check_not_test(u, file, diagnostics);
        }
        Expr::FString(fs) => {
            check_f_string_placeholders(fs, file, diagnostics);
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

fn check_param_name(param: &Parameter, file: File, diagnostics: &mut Vec<Diagnostic>) {
    let name = param.name.as_str();
    if !stdlib_str::is_lowercase(name) {
        diagnostics.push(make_lint_diagnostic(
            &INVALID_ARGUMENT_NAME,
            file,
            param.name.range(),
            format!("Argument name `{name}` should be lowercase"),
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
    let is_classmethod = has_decorator_named(&func.decorator_list, &["classmethod"]);
    let mut skip_first = in_class && !is_static;
    let check_names = !has_decorator_named(&func.decorator_list, &["override"]);

    for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
        if skip_first {
            skip_first = false;
            // N805: instance methods (not @staticmethod, not @classmethod)
            // must name their first parameter `self`. The receiver is
            // exempt from the annotation and default checks.
            if !is_classmethod && pwd.parameter.name.as_str() != "self" {
                diagnostics.push(make_lint_diagnostic(
                    &INVALID_FIRST_ARGUMENT_NAME_FOR_METHOD,
                    file,
                    pwd.parameter.name.range(),
                    "First argument of an instance method should be named `self`".to_owned(),
                ));
            }
            continue;
        }
        check_param_annotation(&pwd.parameter, "", file, diagnostics);
        check_param_default(pwd, file, diagnostics, level);
        if check_names {
            check_param_name(&pwd.parameter, file, diagnostics);
        }
    }

    for pwd in &params.kwonlyargs {
        check_param_annotation(&pwd.parameter, "", file, diagnostics);
        check_param_default(pwd, file, diagnostics, level);
        if check_names {
            check_param_name(&pwd.parameter, file, diagnostics);
        }
    }

    if let Some(vararg) = &params.vararg {
        check_param_annotation(vararg, "*", file, diagnostics);
        if check_names {
            check_param_name(vararg, file, diagnostics);
        }
    }

    if let Some(kwarg) = &params.kwarg {
        check_param_annotation(kwarg, "**", file, diagnostics);
        if check_names {
            check_param_name(kwarg, file, diagnostics);
        }
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

/// Returns true if `decorators` contains a bare-name or attribute decorator
/// whose final identifier matches one of `names`. Matches both `@override`
/// and `@typing.override`, etc.
fn has_decorator_named(decorators: &[Decorator], names: &[&str]) -> bool {
    decorators.iter().any(|d| match &d.expression {
        Expr::Name(n) => names.contains(&n.id.as_str()),
        Expr::Attribute(a) => names.contains(&a.attr.as_str()),
        _ => false,
    })
}

/// N802: function names should be lowercase.
fn check_function_name(func: &StmtFunctionDef, file: File, diagnostics: &mut Vec<Diagnostic>) {
    let name = func.name.as_str();
    if stdlib_str::is_lowercase(name) {
        return;
    }
    if has_decorator_named(&func.decorator_list, &["override", "overload"]) {
        return;
    }
    diagnostics.push(make_lint_diagnostic(
        &INVALID_FUNCTION_NAME,
        file,
        func.name.range(),
        format!("Function name `{name}` should be lowercase"),
    ));
}

/// Returns true if the class directly subclasses `Enum`, `IntEnum`,
/// `StrEnum`, `Flag`, or `IntFlag` (matched by base name only — students
/// don't typically alias these).
fn is_enum_class(cls: &StmtClassDef) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|arg| {
            matches!(arg, Expr::Name(name)
                if matches!(name.id.as_str(),
                    "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag"))
        })
    })
}

/// Members of an `Enum` subclass must be `UPPER_CASE`. Method definitions
/// on the enum class are skipped — they're regular functions and follow the
/// usual snake_case rule via `check_function_name`.
fn check_enum_member_names(cls: &StmtClassDef, file: File, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in &cls.body {
        let (target, range) = match stmt {
            Stmt::Assign(StmtAssign { targets, .. }) => match targets.as_slice() {
                [Expr::Name(n)] => (n.id.as_str(), n.range()),
                _ => continue,
            },
            Stmt::AnnAssign(ann) => match ann.target.as_ref() {
                Expr::Name(n) => (n.id.as_str(), n.range()),
                _ => continue,
            },
            _ => continue,
        };
        if !stdlib_str::is_uppercase(target) {
            diagnostics.push(make_lint_diagnostic(
                &NON_UPPERCASE_ENUM_MEMBER,
                file,
                range,
                format!("Enum member `{target}` should be uppercase"),
            ));
        }
    }
}

/// N801: class names should use the `CapWords` convention.
fn check_class_name(cls: &StmtClassDef, file: File, diagnostics: &mut Vec<Diagnostic>) {
    let name = cls.name.as_str();
    let stripped = name.trim_start_matches('_');
    if stripped.chars().next().is_some_and(char::is_uppercase) && !stripped.contains('_') {
        return;
    }
    diagnostics.push(make_lint_diagnostic(
        &INVALID_CLASS_NAME,
        file,
        cls.name.range(),
        format!("Class name `{name}` should use CapWords convention"),
    ));
}

/// N806: variables assigned inside a function should be lowercase.
///
/// Walks tuple/list unpacking targets recursively. Attribute and subscript
/// targets (`self.x`, `a[0]`) are not name introductions, so they're skipped.
fn check_assign_target_names(target: &Expr, file: File, diagnostics: &mut Vec<Diagnostic>) {
    match target {
        Expr::Name(n) => {
            let name = n.id.as_str();
            if !stdlib_str::is_lowercase(name) {
                diagnostics.push(make_lint_diagnostic(
                    &NON_LOWERCASE_VARIABLE_IN_FUNCTION,
                    file,
                    n.range(),
                    format!("Variable `{name}` in function should be lowercase"),
                ));
            }
        }
        Expr::Tuple(t) => {
            for v in &t.elts {
                check_assign_target_names(v, file, diagnostics);
            }
        }
        Expr::List(l) => {
            for v in &l.elts {
                check_assign_target_names(v, file, diagnostics);
            }
        }
        Expr::Starred(s) => check_assign_target_names(&s.value, file, diagnostics),
        _ => {}
    }
}

/// E711 / E712: flag `== None`, `!= None`, `== True`, `== False`, etc.
///
/// Walks each `(op, comparator)` pair, including the implicit pair formed
/// by `compare.left` and `compare.ops[0] / compare.comparators[0]`. A
/// chained comparison like `a == None == b` would visit the middle `None`
/// from both pairs, so we dedupe by source position.
fn check_literal_comparisons(
    cmp: &ruff_python_ast::ExprCompare,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut emitted: Vec<TextSize> = Vec::new();
    let mut prev = cmp.left.as_ref();
    for (op, right) in cmp.ops.iter().zip(cmp.comparators.iter()) {
        if matches!(op, CmpOp::Eq | CmpOp::NotEq) {
            for side in [prev, right] {
                let start = side.range().start();
                if emitted.contains(&start) {
                    continue;
                }
                if matches!(side, Expr::NoneLiteral(_)) {
                    diagnostics.push(make_lint_diagnostic(
                        &NONE_COMPARISON,
                        file,
                        side.range(),
                        match op {
                            CmpOp::Eq => "Comparison to `None` should be `is None`".to_owned(),
                            _ => "Comparison to `None` should be `is not None`".to_owned(),
                        },
                    ));
                    emitted.push(start);
                } else if matches!(side, Expr::BooleanLiteral(_)) {
                    diagnostics.push(make_lint_diagnostic(
                        &TRUE_FALSE_COMPARISON,
                        file,
                        side.range(),
                        "Comparison to `True` / `False` is redundant; \
                         use the value (or `not value`) directly"
                            .to_owned(),
                    ));
                    emitted.push(start);
                }
            }
        }
        prev = right;
    }
}

/// E713 / E714: `not <a> in <b>` → `<a> not in <b>`; `not <a> is <b>` →
/// `<a> is not <b>`. Only fires for a single-op comparison (chained like
/// `not a in b in c` is too unusual to flag with a clear suggestion).
fn check_not_test(
    unary: &ruff_python_ast::ExprUnaryOp,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Compare(cmp) = unary.operand.as_ref() else {
        return;
    };
    let [op] = cmp.ops.as_ref() else {
        return;
    };
    match op {
        CmpOp::In => diagnostics.push(make_lint_diagnostic(
            &NOT_IN_TEST,
            file,
            unary.range(),
            "Test for membership should be `not in`".to_owned(),
        )),
        CmpOp::Is => diagnostics.push(make_lint_diagnostic(
            &NOT_IS_TEST,
            file,
            unary.range(),
            "Test for object identity should be `is not`".to_owned(),
        )),
        _ => {}
    }
}

/// F541: an f-string with no interpolations is just a regular string —
/// the `f` prefix is misleading.
fn check_f_string_placeholders(
    expr: &ruff_python_ast::ExprFString,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let has_placeholder = expr.value.f_strings().any(|fs| {
        fs.elements
            .iter()
            .any(ruff_python_ast::InterpolatedStringElement::is_interpolation)
    });
    if !has_placeholder {
        diagnostics.push(make_lint_diagnostic(
            &F_STRING_MISSING_PLACEHOLDERS,
            file,
            expr.range(),
            "f-string has no placeholders; remove the `f` prefix".to_owned(),
        ));
    }
}

/// SIM103: `if cond: return True else: return False` (or the inverse) —
/// just `return cond` or `return not cond`.
fn check_needless_bool(
    if_stmt: &ruff_python_ast::StmtIf,
    file: File,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Need exactly one `else:` clause (no `elif`).
    let [else_clause] = if_stmt.elif_else_clauses.as_slice() else {
        return;
    };
    if else_clause.test.is_some() {
        return;
    }
    let if_value = single_return_bool(&if_stmt.body);
    let else_value = single_return_bool(&else_clause.body);
    let (Some(a), Some(b)) = (if_value, else_value) else {
        return;
    };
    if a == b {
        return;
    }
    diagnostics.push(make_lint_diagnostic(
        &NEEDLESS_BOOL,
        file,
        if_stmt.range(),
        "Return the condition directly instead of `if cond: return True else: return False`"
            .to_owned(),
    ));
}

/// If `body` is a single `return <True|False>` statement, return that
/// boolean value; otherwise `None`.
fn single_return_bool(body: &[Stmt]) -> Option<bool> {
    let [Stmt::Return(ret)] = body else {
        return None;
    };
    let value = ret.value.as_deref()?;
    let Expr::BooleanLiteral(lit) = value else {
        return None;
    };
    Some(lit.value)
}

/// F841: report local variables that are assigned but never read.
///
/// Naive AST-based use-def via [`UnusedWalker`]. We do NOT recurse into
/// nested function/class bodies — they have their own scope. As a
/// consequence, an outer binding captured by a nested closure looks
/// unused; that's a known false positive but rare in the student code
/// this rule targets (only enabled at levels 0–3).
fn check_unused_variables(body: &[Stmt], file: File, diagnostics: &mut Vec<Diagnostic>) {
    let mut walker = UnusedWalker::default();
    walker.visit_body(body);
    let UnusedWalker { bindings, uses } = walker;
    let mut reported: HashSet<&str> = HashSet::new();
    for (name, range) in bindings {
        if name.starts_with('_') || uses.contains(name) || !reported.insert(name) {
            continue;
        }
        diagnostics.push(make_lint_diagnostic(
            &UNUSED_VARIABLE,
            file,
            range,
            format!("Local variable `{name}` is assigned but never used"),
        ));
    }
}

#[derive(Default)]
struct UnusedWalker<'a> {
    bindings: Vec<(&'a str, TextRange)>,
    uses: HashSet<&'a str>,
}

impl<'a> UnusedWalker<'a> {
    /// Record every `Name` reachable through tuple/list/starred unpacking
    /// as a binding. Attribute / subscript targets aren't local bindings.
    fn add_target(&mut self, target: &'a Expr) {
        match target {
            Expr::Name(n) => self.bindings.push((n.id.as_str(), n.range())),
            Expr::Tuple(t) => t.elts.iter().for_each(|v| self.add_target(v)),
            Expr::List(l) => l.elts.iter().for_each(|v| self.add_target(v)),
            Expr::Starred(s) => self.add_target(&s.value),
            _ => {}
        }
    }
}

impl<'a> Visitor<'a> for UnusedWalker<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Assign(a) => {
                for t in &a.targets {
                    self.add_target(t);
                }
                self.visit_expr(&a.value);
            }
            Stmt::AnnAssign(a) => {
                self.add_target(&a.target);
                if let Some(v) = &a.value {
                    self.visit_expr(v);
                }
                // Skip the annotation: we don't track whether type names are
                // "used" — that's the type checker's concern.
            }
            Stmt::AugAssign(a) => {
                // `x += 1` reads then writes x. The target's `Name` carries
                // ctx=Store, so the default visitor would skip it; record
                // the read explicitly here.
                if let Expr::Name(n) = a.target.as_ref() {
                    self.uses.insert(n.id.as_str());
                } else {
                    self.visit_expr(&a.target);
                }
                self.visit_expr(&a.value);
            }
            Stmt::For(f) => {
                self.add_target(&f.target);
                self.visit_expr(&f.iter);
                self.visit_body(&f.body);
                self.visit_body(&f.orelse);
            }
            Stmt::FunctionDef(func) => {
                // Nested def: name is a binding in the enclosing scope, but
                // body / parameters / return annotation live in their own
                // scope so we don't visit them.
                self.bindings.push((func.name.as_str(), func.name.range()));
                for d in &func.decorator_list {
                    self.visit_decorator(d);
                }
            }
            Stmt::ClassDef(cls) => {
                self.bindings.push((cls.name.as_str(), cls.name.range()));
                for d in &cls.decorator_list {
                    self.visit_decorator(d);
                }
                if let Some(args) = &cls.arguments {
                    self.visit_arguments(args);
                }
                // Skip body — separate scope.
            }
            Stmt::With(w) => {
                for item in &w.items {
                    self.visit_expr(&item.context_expr);
                    if let Some(v) = &item.optional_vars {
                        self.add_target(v);
                    }
                }
                self.visit_body(&w.body);
            }
            Stmt::Try(t) => {
                self.visit_body(&t.body);
                for handler in &t.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    if let Some(typ) = &h.type_ {
                        self.visit_expr(typ);
                    }
                    if let Some(name) = &h.name {
                        self.bindings.push((name.as_str(), name.range()));
                    }
                    self.visit_body(&h.body);
                }
                self.visit_body(&t.orelse);
                self.visit_body(&t.finalbody);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            // Store ctx is handled by `add_target` at the parent; Del / Invalid no-op.
            Expr::Name(n) if n.ctx == ExprContext::Load => {
                self.uses.insert(n.id.as_str());
            }
            Expr::Name(_) => {}
            Expr::Named(named) => {
                // Walrus `(target := value)` binds target and reads value.
                self.add_target(&named.target);
                self.visit_expr(&named.value);
            }
            _ => walk_expr(self, expr),
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
