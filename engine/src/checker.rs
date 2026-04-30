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
use ty_python_semantic::lint::LintMetadata;
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
    let parsed = parsed_module(db, file);
    let module = parsed.load(db);
    let model = SemanticModel::new(db, file);
    let mut checker = Checker {
        file,
        model: &model,
        level,
        in_repl,
        in_class: false,
        in_function: false,
        in_doctest: false,
        diagnostics: Vec::new(),
    };
    checker.visit_body(module.suite());
    checker.diagnostics
}

/// Walks the AST collecting diagnostics. Contextual flags (`in_class`,
/// `in_function`, `in_doctest`) flip when entering nested scopes and are
/// saved/restored by the visitor.
struct Checker<'a> {
    file: File,
    model: &'a SemanticModel<'a>,
    level: Level,
    in_repl: bool,
    in_class: bool,
    in_function: bool,
    in_doctest: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn push(&mut self, lint: &LintMetadata, range: TextRange, message: String) {
        self.diagnostics
            .push(make_lint_diagnostic(lint, self.file, range, message));
    }

    fn forbidden(&mut self, range: TextRange, label: &str, min_level: Level) {
        self.push(
            &FORBIDDEN_CONSTRUCT,
            range,
            forbidden_msg(label, self.level, min_level),
        );
    }

    /// Check that `expr` (used in a boolean context) has type `bool`.
    ///
    /// Skips `and`/`or`/`not` — their operands are checked individually as
    /// they are each in a boolean context, so reporting on the compound
    /// expression would be redundant. Only applies at levels 0–3.
    fn check_bool_ctx(&mut self, expr: &Expr) {
        if self.level >= Level::Classes {
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
        let Some(ty) = expr.inferred_type(self.model) else {
            return;
        };
        let bool_ty = KnownClass::Bool.to_instance(self.model.db());
        if !ty.is_assignable_to(self.model.db(), bool_ty) {
            self.push(
                &NON_BOOLEAN_CONDITION,
                expr.range(),
                format!(
                    "Condition has type `{}`, expected `bool`",
                    ty.display(self.model.db())
                ),
            );
        }
    }

    /// Reject an arithmetic operand whose inferred type is `bool`. Only
    /// applies at teaching levels 0–3.
    fn reject_bool_arith_operand(&mut self, expr: &Expr) {
        if self.level >= Level::Classes {
            return;
        }
        let Some(ty) = expr.inferred_type(self.model) else {
            return;
        };
        let bool_ty = KnownClass::Bool.to_instance(self.model.db());
        if ty.is_assignable_to(self.model.db(), bool_ty) {
            self.push(
                &BOOL_IN_ARITHMETIC,
                expr.range(),
                format!(
                    "Operand has type `{}`; `bool` is not allowed in arithmetic expressions",
                    ty.display(self.model.db())
                ),
            );
        }
    }

    /// Check a function definition for missing parameter and return
    /// annotations, plus N803/N805 naming and FORBIDDEN_DEFAULT_ARG.
    fn check_function(&mut self, func: &StmtFunctionDef) {
        let params = &func.parameters;
        // Skip the first positional parameter in a method (the implicit
        // receiver: self/cls), unless decorated with @staticmethod.
        let is_static = func
            .decorator_list
            .iter()
            .any(|d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "staticmethod"));
        let is_classmethod = has_decorator_named(&func.decorator_list, &["classmethod"]);
        let mut skip_first = self.in_class && !is_static;
        let check_names = !has_decorator_named(&func.decorator_list, &["override"]);

        for pwd in params.posonlyargs.iter().chain(params.args.iter()) {
            if skip_first {
                skip_first = false;
                // N805: instance methods (not @staticmethod, not
                // @classmethod) must name their first parameter `self`. The
                // receiver is exempt from the annotation and default checks.
                if !is_classmethod && pwd.parameter.name.as_str() != "self" {
                    self.push(
                        &INVALID_FIRST_ARGUMENT_NAME_FOR_METHOD,
                        pwd.parameter.name.range(),
                        "First argument of an instance method should be named `self`".to_owned(),
                    );
                }
                continue;
            }
            self.check_param_annotation(&pwd.parameter, "");
            self.check_param_default(pwd);
            if check_names {
                self.check_param_name(&pwd.parameter);
            }
        }

        for pwd in &params.kwonlyargs {
            self.check_param_annotation(&pwd.parameter, "");
            self.check_param_default(pwd);
            if check_names {
                self.check_param_name(&pwd.parameter);
            }
        }

        if let Some(vararg) = &params.vararg {
            self.check_param_annotation(vararg, "*");
            if check_names {
                self.check_param_name(vararg);
            }
        }

        if let Some(kwarg) = &params.kwarg {
            self.check_param_annotation(kwarg, "**");
            if check_names {
                self.check_param_name(kwarg);
            }
        }

        if func.returns.is_none() {
            self.push(
                &MISSING_RETURN_ANNOTATION,
                func.name.range(),
                format!(
                    "Function `{}` is missing a return type annotation",
                    func.name.as_str()
                ),
            );
        }
    }

    fn check_param_annotation(&mut self, param: &Parameter, prefix: &str) {
        if param.annotation.is_none() {
            self.push(
                &MISSING_PARAMETER_ANNOTATION,
                param.range(),
                format!(
                    "Parameter `{prefix}{}` is missing a type annotation",
                    param.name.as_str()
                ),
            );
        }
    }

    fn check_param_default(&mut self, pwd: &ruff_python_ast::ParameterWithDefault) {
        if self.level >= Level::Classes {
            return;
        }
        if let Some(default) = &pwd.default {
            self.push(
                &FORBIDDEN_DEFAULT_ARG,
                default.range(),
                format!(
                    "Default value for parameter `{}` is not allowed at level {}, requires level {}",
                    pwd.parameter.name.as_str(),
                    self.level,
                    Level::Classes,
                ),
            );
        }
    }

    fn check_param_name(&mut self, param: &Parameter) {
        let name = param.name.as_str();
        if !stdlib_str::is_lowercase(name) {
            self.push(
                &INVALID_ARGUMENT_NAME,
                param.name.range(),
                format!("Argument name `{name}` should be lowercase"),
            );
        }
    }

    /// N802: function names should be lowercase.
    fn check_function_name(&mut self, func: &StmtFunctionDef) {
        let name = func.name.as_str();
        if stdlib_str::is_lowercase(name) {
            return;
        }
        if has_decorator_named(&func.decorator_list, &["override", "overload"]) {
            return;
        }
        self.push(
            &INVALID_FUNCTION_NAME,
            func.name.range(),
            format!("Function name `{name}` should be lowercase"),
        );
    }

    /// N801: class names should use the `CapWords` convention.
    fn check_class_name(&mut self, cls: &StmtClassDef) {
        let name = cls.name.as_str();
        let stripped = name.trim_start_matches('_');
        if stripped.chars().next().is_some_and(char::is_uppercase) && !stripped.contains('_') {
            return;
        }
        self.push(
            &INVALID_CLASS_NAME,
            cls.name.range(),
            format!("Class name `{name}` should use CapWords convention"),
        );
    }

    /// Check the direct statements of a class body for unannotated assignments.
    fn check_class_body(&mut self, class_def: &StmtClassDef) {
        for stmt in &class_def.body {
            if let Stmt::Assign(StmtAssign { targets, .. }) = stmt {
                for target in targets {
                    if let Expr::Name(name) = target {
                        self.push(
                            &MISSING_ATTRIBUTE_ANNOTATION,
                            name.range(),
                            format!(
                                "Class attribute `{}` is missing a type annotation",
                                name.id.as_str()
                            ),
                        );
                    }
                }
            }
        }
    }

    /// Members of an `Enum` subclass must be `UPPER_CASE`. Method
    /// definitions on the enum class are skipped — they're regular
    /// functions and follow the usual snake_case rule.
    fn check_enum_member_names(&mut self, cls: &StmtClassDef) {
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
                self.push(
                    &NON_UPPERCASE_ENUM_MEMBER,
                    range,
                    format!("Enum member `{target}` should be uppercase"),
                );
            }
        }
    }

    /// N806: variables assigned inside a function should be lowercase.
    /// Walks tuple/list unpacking targets recursively. Attribute and
    /// subscript targets (`self.x`, `a[0]`) are not name introductions.
    fn check_assign_target_names(&mut self, target: &Expr) {
        match target {
            Expr::Name(n) => {
                let name = n.id.as_str();
                if !stdlib_str::is_lowercase(name) {
                    self.push(
                        &NON_LOWERCASE_VARIABLE_IN_FUNCTION,
                        n.range(),
                        format!("Variable `{name}` in function should be lowercase"),
                    );
                }
            }
            Expr::Tuple(t) => t
                .elts
                .iter()
                .for_each(|v| self.check_assign_target_names(v)),
            Expr::List(l) => l
                .elts
                .iter()
                .for_each(|v| self.check_assign_target_names(v)),
            Expr::Starred(s) => self.check_assign_target_names(&s.value),
            _ => {}
        }
    }

    /// E711 / E712: flag `== None`, `!= None`, `== True`, `== False`, etc.
    /// A chained comparison like `a == None == b` would visit the middle
    /// `None` from both pairs, so we dedupe by source position.
    fn check_literal_comparisons(&mut self, cmp: &ruff_python_ast::ExprCompare) {
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
                        self.push(
                            &NONE_COMPARISON,
                            side.range(),
                            match op {
                                CmpOp::Eq => "Comparison to `None` should be `is None`".to_owned(),
                                _ => "Comparison to `None` should be `is not None`".to_owned(),
                            },
                        );
                        emitted.push(start);
                    } else if matches!(side, Expr::BooleanLiteral(_)) {
                        self.push(
                            &TRUE_FALSE_COMPARISON,
                            side.range(),
                            "Comparison to `True` / `False` is redundant; \
                             use the value (or `not value`) directly"
                                .to_owned(),
                        );
                        emitted.push(start);
                    }
                }
            }
            prev = right;
        }
    }

    /// E713 / E714: `not <a> in <b>` → `<a> not in <b>`; `not <a> is <b>` →
    /// `<a> is not <b>`. Only fires for a single-op comparison.
    fn check_not_test(&mut self, unary: &ruff_python_ast::ExprUnaryOp) {
        let Expr::Compare(cmp) = unary.operand.as_ref() else {
            return;
        };
        let [op] = cmp.ops.as_ref() else {
            return;
        };
        match op {
            CmpOp::In => self.push(
                &NOT_IN_TEST,
                unary.range(),
                "Test for membership should be `not in`".to_owned(),
            ),
            CmpOp::Is => self.push(
                &NOT_IS_TEST,
                unary.range(),
                "Test for object identity should be `is not`".to_owned(),
            ),
            _ => {}
        }
    }

    /// F541: an f-string with no interpolations is just a regular string —
    /// the `f` prefix is misleading.
    fn check_f_string_placeholders(&mut self, expr: &ruff_python_ast::ExprFString) {
        let has_placeholder = expr.value.f_strings().any(|fs| {
            fs.elements
                .iter()
                .any(ruff_python_ast::InterpolatedStringElement::is_interpolation)
        });
        if !has_placeholder {
            self.push(
                &F_STRING_MISSING_PLACEHOLDERS,
                expr.range(),
                "f-string has no placeholders; remove the `f` prefix".to_owned(),
            );
        }
    }

    /// SIM103: `if cond: return True else: return False` (or the inverse) —
    /// just `return cond` or `return not cond`.
    fn check_needless_bool(&mut self, if_stmt: &ruff_python_ast::StmtIf) {
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
        self.push(
            &NEEDLESS_BOOL,
            if_stmt.range(),
            "Return the condition directly instead of `if cond: return True else: return False`"
                .to_owned(),
        );
    }
}

impl<'a> Visitor<'a> for Checker<'_> {
    fn visit_body(&mut self, body: &'a [Stmt]) {
        // PIE790: report `pass` statements that aren't the only statement
        // in their body (i.e. they are unnecessary).
        if body.len() > 1 {
            for stmt in body {
                if matches!(stmt, Stmt::Pass(_)) {
                    self.push(
                        &UNNECESSARY_PASS,
                        stmt.range(),
                        "Unnecessary `pass` statement".to_owned(),
                    );
                }
            }
        }
        for stmt in body {
            self.visit_stmt(stmt);
        }
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(func) => {
                if func.is_async && self.level < Level::Full {
                    self.forbidden(func.range(), "`async def`", Level::Full);
                }
                self.check_function(func);
                let synthetic = crate::doctests::is_synthetic_fn_name(func.name.as_str());
                if !synthetic {
                    self.check_function_name(func);
                }
                if self.level <= Level::Repetition && !synthetic {
                    check_unused_variables(&func.body, self.file, &mut self.diagnostics);
                }
                let prev = (self.in_class, self.in_function, self.in_doctest);
                self.in_class = false;
                self.in_function = true;
                self.in_doctest = self.in_doctest || synthetic;
                self.visit_body(&func.body);
                (self.in_class, self.in_function, self.in_doctest) = prev;
                for d in &func.decorator_list {
                    self.visit_decorator(d);
                }
            }
            Stmt::ClassDef(cls) => {
                self.check_class_name(cls);
                if self.level < Level::UserTypes {
                    self.push(
                        &FORBIDDEN_CLASS,
                        cls.name.range(),
                        forbidden_msg("`class`", self.level, Level::UserTypes),
                    );
                } else {
                    if self.level < Level::Classes {
                        for body_stmt in &cls.body {
                            if let Stmt::FunctionDef(func) = body_stmt {
                                self.push(
                                    &FORBIDDEN_CLASS_METHOD,
                                    func.name.range(),
                                    forbidden_msg("Methods in classes", self.level, Level::Classes),
                                );
                            }
                        }
                    }
                    // Skip annotation check for Enum subclasses — Enum
                    // members don't need (and shouldn't have) annotations.
                    if is_enum_class(cls) {
                        self.check_enum_member_names(cls);
                    } else {
                        self.check_class_body(cls);
                    }
                }
                let prev = (self.in_class, self.in_function);
                self.in_class = true;
                self.in_function = false;
                self.visit_body(&cls.body);
                (self.in_class, self.in_function) = prev;
                for d in &cls.decorator_list {
                    self.visit_decorator(d);
                }
            }
            Stmt::For(for_stmt) => {
                if for_stmt.is_async && self.level < Level::Full {
                    self.forbidden(for_stmt.range(), "`async for`", Level::Full);
                } else if self.level < Level::Repetition {
                    self.push(
                        &FORBIDDEN_LOOP,
                        for_stmt.range(),
                        forbidden_msg("`for` loop", self.level, Level::Repetition),
                    );
                }
                self.visit_expr(&for_stmt.iter);
                if self.in_function {
                    self.check_assign_target_names(&for_stmt.target);
                }
                self.visit_body(&for_stmt.body);
                self.visit_body(&for_stmt.orelse);
            }
            Stmt::While(while_stmt) => {
                if self.level < Level::Repetition {
                    self.push(
                        &FORBIDDEN_LOOP,
                        while_stmt.range(),
                        forbidden_msg("`while` loop", self.level, Level::Repetition),
                    );
                }
                self.check_bool_ctx(&while_stmt.test);
                self.visit_expr(&while_stmt.test);
                self.visit_body(&while_stmt.body);
                self.visit_body(&while_stmt.orelse);
            }
            Stmt::If(if_stmt) => {
                if self.level < Level::Selection {
                    self.push(
                        &FORBIDDEN_SELECTION,
                        if_stmt.range(),
                        forbidden_msg("`if`", self.level, Level::Selection),
                    );
                }
                self.check_needless_bool(if_stmt);
                self.check_bool_ctx(&if_stmt.test);
                self.visit_expr(&if_stmt.test);
                self.visit_body(&if_stmt.body);
                for clause in &if_stmt.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        self.check_bool_ctx(test);
                        self.visit_expr(test);
                    }
                    self.visit_body(&clause.body);
                }
            }
            Stmt::Match(match_stmt) => {
                if self.level < Level::UserTypes {
                    self.push(
                        &FORBIDDEN_MATCH,
                        match_stmt.range(),
                        forbidden_msg("`match`", self.level, Level::UserTypes),
                    );
                }
                self.visit_expr(&match_stmt.subject);
                for case in &match_stmt.cases {
                    if let Some(guard) = &case.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_body(&case.body);
                }
            }
            Stmt::With(with_stmt) => {
                if self.level < Level::Full {
                    self.forbidden(with_stmt.range(), "`with`", Level::Full);
                }
                for item in &with_stmt.items {
                    self.visit_expr(&item.context_expr);
                }
                self.visit_body(&with_stmt.body);
            }
            Stmt::Try(try_stmt) => {
                if self.level < Level::Full {
                    self.forbidden(try_stmt.range(), "`try`", Level::Full);
                }
                self.visit_body(&try_stmt.body);
                for handler in &try_stmt.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.visit_body(&h.body);
                }
                self.visit_body(&try_stmt.orelse);
                self.visit_body(&try_stmt.finalbody);
            }
            Stmt::Global(s) if self.level < Level::Full => {
                self.forbidden(s.range(), "`global`", Level::Full);
            }
            Stmt::Nonlocal(s) if self.level < Level::Full => {
                self.forbidden(s.range(), "`nonlocal`", Level::Full);
            }
            Stmt::Delete(s) if self.level < Level::Full => {
                self.forbidden(s.range(), "`del`", Level::Full);
            }
            Stmt::AugAssign(aug) => {
                if self.level < Level::Repetition {
                    self.push(
                        &FORBIDDEN_AUG_ASSIGN,
                        aug.range(),
                        forbidden_msg("Augmented assignment", self.level, Level::Repetition),
                    );
                }
                if is_arithmetic_op(aug.op) {
                    self.reject_bool_arith_operand(&aug.target);
                    self.reject_bool_arith_operand(&aug.value);
                }
                self.visit_expr(&aug.value);
            }
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.visit_expr(value);
                }
            }
            Stmt::Assign(assign) => {
                if self.in_function {
                    for target in &assign.targets {
                        self.check_assign_target_names(target);
                    }
                }
                self.visit_expr(&assign.value);
            }
            Stmt::AnnAssign(ann) => {
                if self.in_function {
                    self.check_assign_target_names(&ann.target);
                }
                if let Some(value) = &ann.value {
                    self.visit_expr(value);
                }
            }
            Stmt::Expr(expr_stmt) => {
                if !self.in_repl
                    && !self.in_doctest
                    && self.level < Level::Classes
                    && !matches!(
                        &*expr_stmt.value,
                        Expr::Call(_) | Expr::StringLiteral(_) | Expr::EllipsisLiteral(_)
                    )
                {
                    self.push(
                        &BARE_EXPRESSION,
                        expr_stmt.range(),
                        "Expression statement has no effect; did you forget `=` or `print(...)`?"
                            .to_owned(),
                    );
                }
                self.visit_expr(&expr_stmt.value);
            }
            Stmt::Assert(assert_stmt) => {
                self.check_bool_ctx(&assert_stmt.test);
                self.visit_expr(&assert_stmt.test);
                if let Some(msg) = &assert_stmt.msg {
                    self.visit_expr(msg);
                }
            }
            Stmt::Raise(raise) => {
                if let Some(exc) = &raise.exc {
                    self.visit_expr(exc);
                }
            }
            // Always allowed (or allowed at level 5): no checks, no
            // sub-expressions to recurse into.
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

    fn visit_expr(&mut self, expr: &'a Expr) {
        // Forbidden constructs and expression-level lints.
        match expr {
            Expr::List(e) if self.level < Level::Repetition => self.push(
                &FORBIDDEN_COLLECTION_LITERAL,
                e.range(),
                forbidden_msg("List literal", self.level, Level::Repetition),
            ),
            Expr::Tuple(e) if self.level < Level::Repetition => self.push(
                &FORBIDDEN_COLLECTION_LITERAL,
                e.range(),
                forbidden_msg("Tuple literal", self.level, Level::Repetition),
            ),
            Expr::Dict(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COLLECTION_LITERAL,
                e.range(),
                forbidden_msg("Dict literal", self.level, Level::Classes),
            ),
            Expr::Set(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COLLECTION_LITERAL,
                e.range(),
                forbidden_msg("Set literal", self.level, Level::Classes),
            ),
            Expr::ListComp(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COMPREHENSION,
                e.range(),
                forbidden_msg("List comprehension", self.level, Level::Classes),
            ),
            Expr::SetComp(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COMPREHENSION,
                e.range(),
                forbidden_msg("Set comprehension", self.level, Level::Classes),
            ),
            Expr::DictComp(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COMPREHENSION,
                e.range(),
                forbidden_msg("Dict comprehension", self.level, Level::Classes),
            ),
            Expr::Generator(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_COMPREHENSION,
                e.range(),
                forbidden_msg("Generator expression", self.level, Level::Classes),
            ),
            Expr::Lambda(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_LAMBDA,
                e.range(),
                forbidden_msg("`lambda`", self.level, Level::Classes),
            ),
            Expr::Compare(cmp) if self.level < Level::Classes && cmp.comparators.len() > 1 => {
                self.push(
                    &CHAINED_COMPARISON,
                    cmp.range(),
                    "Chained comparison is not allowed; split with `and` / `or`".to_owned(),
                );
            }
            Expr::Compare(cmp) => self.check_literal_comparisons(cmp),
            Expr::UnaryOp(u) if u.op == UnaryOp::Not => self.check_not_test(u),
            Expr::FString(fs) => self.check_f_string_placeholders(fs),
            Expr::Yield(y) if self.level < Level::Full => {
                self.forbidden(y.range(), "`yield`", Level::Full);
            }
            Expr::YieldFrom(y) if self.level < Level::Full => {
                self.forbidden(y.range(), "`yield from`", Level::Full);
            }
            Expr::Await(a) if self.level < Level::Full => {
                self.forbidden(a.range(), "`await`", Level::Full);
            }
            _ => {}
        }

        // Position-specific contextual checks (bool ctx, arithmetic operands).
        match expr {
            Expr::BoolOp(e) => {
                for v in &e.values {
                    self.check_bool_ctx(v);
                }
            }
            Expr::BinOp(e) if is_arithmetic_op(e.op) => {
                self.reject_bool_arith_operand(&e.left);
                self.reject_bool_arith_operand(&e.right);
            }
            Expr::UnaryOp(u) => match u.op {
                UnaryOp::Not => self.check_bool_ctx(&u.operand),
                UnaryOp::UAdd | UnaryOp::USub => self.reject_bool_arith_operand(&u.operand),
                UnaryOp::Invert => {}
            },
            Expr::If(e) => self.check_bool_ctx(&e.test),
            _ => {}
        }

        walk_expr(self, expr);
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
    lint: &LintMetadata,
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
