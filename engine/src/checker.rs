//! Annotation checker and construct restriction checker for spython.
//!
//! This module checks for missing annotations and forbidden constructs based
//! on the current teaching level.

use ruff_db::diagnostic::{Annotation, Diagnostic, DiagnosticId, Severity, Span};
use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use std::collections::{HashMap, HashSet};

use ruff_python_ast::token::{TokenKind, Tokens};
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_pattern, walk_stmt};
use ruff_python_ast::{
    CmpOp, Comprehension, Decorator, Expr, ExprContext, Operator, Parameter, Parameters, Pattern,
    Stmt, StmtAssign, StmtClassDef, StmtFunctionDef, TypeParams, UnaryOp,
};
use ruff_python_stdlib::str as stdlib_str;
use ruff_text_size::{Ranged, TextRange, TextSize};
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_project::Db;
use ty_python_semantic::lint::LintMetadata;
use ty_python_semantic::types::{KnownClass, Type};
use ty_python_semantic::{HasType, SemanticModel};

use crate::lints::{
    BARE_EXPRESSION, BOOL_IN_ARITHMETIC, CHAINED_COMPARISON, F_STRING_MISSING_PLACEHOLDERS,
    FORBIDDEN_AUG_ASSIGN, FORBIDDEN_CLASS, FORBIDDEN_CLASS_METHOD, FORBIDDEN_COLLECTION_LITERAL,
    FORBIDDEN_COMPREHENSION, FORBIDDEN_CONSTRUCT, FORBIDDEN_DEFAULT_ARG, FORBIDDEN_LAMBDA,
    FORBIDDEN_LOOP, FORBIDDEN_MATCH, FORBIDDEN_SELECTION, INVALID_ARGUMENT_NAME,
    INVALID_CLASS_NAME, INVALID_FIRST_ARGUMENT_NAME_FOR_METHOD, INVALID_FUNCTION_NAME,
    LOCAL_UNKNOWN_TYPE, MISSING_ATTRIBUTE_ANNOTATION, MISSING_PARAMETER_ANNOTATION,
    MISSING_RETURN_ANNOTATION, MISSING_TYPE_PARAMETER, NEEDLESS_BOOL, NON_BOOLEAN_CONDITION,
    NON_LOWERCASE_VARIABLE_IN_FUNCTION, NON_UPPERCASE_ENUM_MEMBER, NONE_COMPARISON, NOT_IN_TEST,
    NOT_IS_TEST, REDEFINED_FUNCTION_OR_CLASS, RESERVED_NAME, SHADOWED_MODULE,
    TRUE_FALSE_COMPARISON, UNAVAILABLE_MODULE, UNNECESSARY_PASS, UNUSED_VARIABLE,
};

/// The start of the names that spython gives to the code it generates.
pub(crate) const RESERVED_PREFIX: &str = "__spython_";

/// The modules of the standard library the interpreter has.
const AVAILABLE_MODULES: &[&str] = &["dataclasses", "enum", "math", "typing"];

/// Teaching level that controls which Python constructs are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// def, return, scalars (int, float, str, bool), string indexing.
    Functions = 0,
    /// Adds: if/elif/else and the conditional expression.
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
/// `repl_input` is where the REPL input starts in the file, and `None` when
/// the file is not REPL input. A bare expression statement is valid in a REPL
/// (its value is displayed) so the `BARE_EXPRESSION` lint is suppressed in
/// that context.
pub fn check_file_annotations(
    db: &dyn Db,
    file: File,
    level: Level,
    repl_input: Option<TextSize>,
) -> Vec<Diagnostic> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let mut checker = Checker {
        file,
        model: &model,
        level,
        repl_input,
        in_class: false,
        in_function: false,
        in_doctest: false,
        diagnostics: Vec::new(),
    };
    checker.visit_body(module.suite());
    checker.check_reserved_names(module.tokens(), &source_text(db, file));
    checker.diagnostics
}

/// Walks the AST collecting diagnostics. Contextual flags (`in_class`,
/// `in_function`, `in_doctest`) flip when entering nested scopes and are
/// saved/restored by the visitor.
struct Checker<'a> {
    file: File,
    model: &'a SemanticModel<'a>,
    level: Level,
    repl_input: Option<TextSize>,
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

    /// Report every name in `tokens` that starts with [`RESERVED_PREFIX`].
    /// A token covers every place a name can be, the attribute after a dot
    /// and the code inside an f-string included.
    fn check_reserved_names(&mut self, tokens: &Tokens, source: &str) {
        for token in tokens {
            if token.kind() != TokenKind::Identifier
                || self.repl_input.is_some_and(|start| token.start() < start)
            {
                continue;
            }
            let name = &source[token.range()];
            if name.starts_with(RESERVED_PREFIX) {
                self.push(
                    &RESERVED_NAME,
                    token.range(),
                    format!(
                        "Names that start with `{RESERVED_PREFIX}` are reserved for spython; \
                         rename `{name}`"
                    ),
                );
            }
        }
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
    /// Skips `and`/`or`/`not`. Their operands are checked individually as
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
        let env = self.model.program_environment();
        let bool_ty = KnownClass::Bool.to_instance(self.model.db(), &env);
        if !ty.is_assignable_to(self.model.db(), &env, bool_ty) {
            self.push(
                &NON_BOOLEAN_CONDITION,
                expr.range(),
                format!(
                    "Condition has type `{}`, expected `bool`",
                    ty.display(self.model.db(), &env)
                ),
            );
        }
    }

    /// Returns `true` if `expr` names a type, so that a subscript of it takes
    /// type arguments, `false` otherwise.
    fn is_type_form(&self, expr: &Expr) -> bool {
        matches!(
            expr.inferred_type(self.model),
            Some(
                Type::ClassLiteral(_)
                    | Type::GenericAlias(_)
                    | Type::SubclassOf(_)
                    | Type::SpecialForm(_)
                    | Type::KnownInstance(_)
                    | Type::TypeAlias(_)
            )
        )
    }

    /// Returns `true` if `cls` is a subclass of `Enum`, directly or not,
    /// `false` otherwise.
    fn is_enum_class(&self, cls: &StmtClassDef) -> bool {
        let Some(ty) = cls.inferred_type(self.model) else {
            return false;
        };
        let db = self.model.db();
        let env = self.model.program_environment();
        let enum_ty = KnownClass::Enum.to_subclass_of(db, &env);
        // A class with a base that ty cannot resolve is assignable to the
        // type of every class, and no class subclasses both `int` and `str`,
        // so the second test keeps such a class out.
        let int_ty = KnownClass::Int.to_subclass_of(db, &env);
        let str_ty = KnownClass::Str.to_subclass_of(db, &env);
        ty.is_assignable_to(db, &env, enum_ty)
            && !(ty.is_assignable_to(db, &env, int_ty) && ty.is_assignable_to(db, &env, str_ty))
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
        let db = self.model.db();
        let env = self.model.program_environment();
        let bool_ty = KnownClass::Bool.to_instance(db, &env);
        // `Any`/`Unknown` are gradual types and are assignable to *every*
        // type, `bool` included, so `is_assignable_to(bool)` alone would
        // misfire on operands ty cannot infer (e.g. `(-1) ** i`, which ty
        // types as `Any`). A genuine `bool`/`Literal[bool]` is assignable to
        // `bool` but never to a disjoint type like `str`, whereas a gradual
        // type is assignable to both. Requiring "assignable to `bool`, not to
        // `str`" thus excludes gradual types while still catching real bools.
        let str_ty = KnownClass::Str.to_instance(db, &env);
        if ty.is_assignable_to(db, &env, bool_ty) && !ty.is_assignable_to(db, &env, str_ty) {
            self.push(
                &BOOL_IN_ARITHMETIC,
                expr.range(),
                format!(
                    "Operand has type `{}`; `bool` is not allowed in arithmetic expressions",
                    ty.display(db, &env)
                ),
            );
        }
    }

    /// Reject `module` when the interpreter would not import what ty resolved
    /// it to: a module of the standard library the interpreter lacks, or a
    /// file beside the program that a module of the interpreter hides. A
    /// module that does not resolve is left to ty.
    fn check_module(&mut self, module: &str, range: TextRange) {
        let package = module.split('.').next().unwrap_or(module);
        let Some(name) = ModuleName::new(package) else {
            return;
        };
        let db = self.model.db();
        let importing = ImportingFile::ResolverFile(db.program_file(self.file).resolver_file(db));
        let Some(resolved) = resolve_module(db, importing, &name) else {
            return;
        };
        // Only a namespace package has no search path, and the standard
        // library has none, so it is a directory beside the file.
        match resolved.search_path(db) {
            Some(path) if path.is_standard_library() => {
                if !AVAILABLE_MODULES.contains(&package) {
                    self.push(
                        &UNAVAILABLE_MODULE,
                        range,
                        format!(
                            "Module `{package}` is not available; a program can import \
                             `dataclasses`, `enum`, `math`, `typing`, `spython` and the \
                             modules beside it"
                        ),
                    );
                }
            }
            Some(path) if !path.is_first_party() => {}
            _ => {
                if crate::run::has_own_package(package) {
                    self.push(
                        &SHADOWED_MODULE,
                        range,
                        format!(
                            "The interpreter imports its own `{package}` and never reads \
                             the one beside the file; rename the file"
                        ),
                    );
                }
            }
        }
    }

    /// Check a function definition for missing parameter and return
    /// annotations, plus N803/N805 naming and FORBIDDEN_DEFAULT_ARG.
    fn check_function(&mut self, func: &StmtFunctionDef) {
        let params = &func.parameters;
        // Skip the first positional parameter in a method (the implicit
        // receiver: self/cls), unless decorated with @staticmethod.
        let is_static = has_decorator_named(&func.decorator_list, &["staticmethod"]);
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

        match &func.returns {
            None => self.push(
                &MISSING_RETURN_ANNOTATION,
                func.name.range(),
                format!(
                    "Function `{}` is missing a return type annotation",
                    func.name.as_str()
                ),
            ),
            Some(ret) => self.check_annotation(ret),
        }
    }

    fn check_param_annotation(&mut self, param: &Parameter, prefix: &str) {
        match &param.annotation {
            None => self.push(
                &MISSING_PARAMETER_ANNOTATION,
                param.range(),
                format!(
                    "Parameter `{prefix}{}` is missing a type annotation",
                    param.name.as_str()
                ),
            ),
            Some(ann) => self.check_annotation(ann),
        }
    }

    /// Walk a type annotation expression and flag bare generic types (only
    /// `list` for now) that appear without a type parameter. Recurses into
    /// `Subscript` slices, union (`BinOp`), tuple, and starred expressions so
    /// nested annotations like `dict[str, list]` and `int | list` are
    /// checked. Does NOT call `visit_expr`, because annotations are type
    /// expressions, not runtime expressions, so the expression-level lints
    /// shouldn't fire here.
    fn check_annotation(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(n) if n.id.as_str() == "list" => {
                self.push(
                    &MISSING_TYPE_PARAMETER,
                    n.range(),
                    "Type `list` requires a parameter, e.g. `list[int]`".to_owned(),
                );
            }
            Expr::Subscript(sub) => {
                // `list[...]` is fine, so don't flag the bare `list` here.
                // Other generic bases (e.g. `dict`) recurse normally so a
                // nested bare `list` still fires. The slice is always
                // recursed into.
                if !matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "list") {
                    self.check_annotation(&sub.value);
                }
                self.check_annotation(&sub.slice);
            }
            Expr::BinOp(b) => {
                self.check_annotation(&b.left);
                self.check_annotation(&b.right);
            }
            Expr::Tuple(t) => {
                for e in &t.elts {
                    self.check_annotation(e);
                }
            }
            Expr::Starred(s) => self.check_annotation(&s.value),
            _ => {}
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
    /// definitions on the enum class are skipped. They're regular
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

    /// Visit the *sub-expressions* of an assignment-style target so that
    /// expression-level lints fire on `arr[lambda: 0] = 1` and similar.
    ///
    /// Tuple / list / starred wrappers are unpacking patterns, not literal
    /// expressions, so they are descended into without being flagged. Plain
    /// `Name` targets carry `ctx=Store` and have nothing to check.
    fn visit_assign_target_subexprs(&mut self, target: &Expr) {
        match target {
            Expr::Tuple(t) => t
                .elts
                .iter()
                .for_each(|e| self.visit_assign_target_subexprs(e)),
            Expr::List(l) => l
                .elts
                .iter()
                .for_each(|e| self.visit_assign_target_subexprs(e)),
            Expr::Starred(s) => self.visit_assign_target_subexprs(&s.value),
            Expr::Subscript(_) | Expr::Attribute(_) => self.visit_expr(target),
            _ => {}
        }
    }

    /// N806: variables assigned inside a function should be lowercase.
    /// Walks tuple/list unpacking targets recursively. Attribute and
    /// subscript targets (`self.x`, `a[0]`) are not name introductions.
    fn check_assign_target_names(&mut self, target: &Expr) {
        match target {
            Expr::Name(n) => self.check_variable_name(n.id.as_str(), n.range()),
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

    /// N806 for one name that a function binds.
    fn check_variable_name(&mut self, name: &str, range: TextRange) {
        if !stdlib_str::is_lowercase(name) {
            self.push(
                &NON_LOWERCASE_VARIABLE_IN_FUNCTION,
                range,
                format!("Variable `{name}` in function should be lowercase"),
            );
        }
    }

    /// E711 / E712: flag `== None`, `!= None`, `== True`, `== False`, etc.
    /// A chained comparison like `a == None == b` would visit the middle
    /// `None` from both pairs, so we dedupe by source position.
    fn check_literal_comparisons(&mut self, cmp: &ruff_python_ast::ExprCompare) {
        let mut emitted: Vec<TextSize> = Vec::new();
        let mut prev = cmp.first_operand();
        for (op, right) in cmp.ops.iter().zip(cmp.comparators()) {
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

    /// F541: an f-string with no interpolations is just a regular string, and
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

    /// Flag every `Name` target in `assign` (recursing through tuple / list /
    /// starred unpacking) with `LOCAL_UNKNOWN_TYPE` when the RHS type is
    /// `Unknown`. Skipped inside synthetic doctest functions because the
    /// generated wrappers don't carry meaningful types.
    fn check_unknown_local(&mut self, assign: &ruff_python_ast::StmtAssign) {
        if self.in_doctest {
            return;
        }
        let Some(ty) = assign.value.inferred_type(self.model) else {
            return;
        };
        if !ty.is_unknown() {
            return;
        }
        for target in &assign.targets {
            self.flag_unknown_local_target(target);
        }
    }

    fn flag_unknown_local_target(&mut self, target: &Expr) {
        match target {
            Expr::Name(n) => self.push(
                &LOCAL_UNKNOWN_TYPE,
                n.range(),
                format!(
                    "Local variable `{}` has Unknown type; add a type annotation",
                    n.id.as_str()
                ),
            ),
            Expr::Tuple(t) => t
                .elts
                .iter()
                .for_each(|e| self.flag_unknown_local_target(e)),
            Expr::List(l) => l
                .elts
                .iter()
                .for_each(|e| self.flag_unknown_local_target(e)),
            Expr::Starred(s) => self.flag_unknown_local_target(&s.value),
            _ => {}
        }
    }

    /// Flag function/class definitions whose name was already defined by an
    /// earlier function/class in the same body. `@overload` stubs and
    /// synthetic doctest functions are ignored. The caller decides when this
    /// runs, which is only at teaching levels 0–3.
    ///
    /// In the REPL a definition from an earlier input does not count, so a
    /// new input can replace a function or a class.
    fn check_redefinitions(&mut self, body: &[Stmt]) {
        let mut defined: HashMap<&str, &'static str> = HashMap::new();
        for stmt in body {
            if self.repl_input.is_some_and(|start| stmt.start() < start) {
                continue;
            }
            let (name, range, kind) = match stmt {
                Stmt::FunctionDef(f) => {
                    if has_decorator_named(&f.decorator_list, &["overload"])
                        || crate::doctests::is_synthetic_fn_name(f.name.as_str())
                    {
                        continue;
                    }
                    (f.name.as_str(), f.name.range(), "Function")
                }
                Stmt::ClassDef(c) => (c.name.as_str(), c.name.range(), "Class"),
                _ => continue,
            };
            if let Some(prev_kind) = defined.insert(name, kind) {
                self.push(
                    &REDEFINED_FUNCTION_OR_CLASS,
                    range,
                    format!(
                        "{kind} `{name}` is redefined; previous {} definition is shadowed",
                        prev_kind.to_lowercase()
                    ),
                );
            }
        }
    }

    /// SIM103: `if cond: return True else: return False` (or the inverse) is
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
        // Flag function/class definitions whose name was already defined by
        // an earlier function/class in the same block. Only at teaching
        // levels 0–3: at level 4+ shadowing is a legitimate technique.
        // `@overload` stubs and synthetic doctest functions are ignored.
        if self.level < Level::Classes {
            self.check_redefinitions(body);
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
                // A default runs where the `def` is, not inside the function.
                for parameter in &func.parameters {
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
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
                    // Skip annotation check for Enum subclasses. Enum
                    // members don't need (and shouldn't have) annotations.
                    if self.is_enum_class(cls) {
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
                // Base classes and keyword arguments are evaluated in the
                // enclosing scope, so visit them after restoring the flags.
                if let Some(args) = &cls.arguments {
                    self.visit_arguments(args);
                }
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
                self.visit_assign_target_subexprs(&for_stmt.target);
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
                    self.visit_pattern(&case.pattern);
                    if let Some(guard) = &case.guard {
                        self.check_bool_ctx(guard);
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
                    if let Some(target) = &item.optional_vars {
                        if self.in_function {
                            self.check_assign_target_names(target);
                        }
                        self.visit_assign_target_subexprs(target);
                    }
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
                    if let Some(type_) = &h.type_ {
                        self.visit_expr(type_);
                    }
                    if self.in_function
                        && let Some(name) = &h.name
                    {
                        self.check_variable_name(name.as_str(), name.range());
                    }
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
                self.visit_assign_target_subexprs(&aug.target);
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
                    self.check_unknown_local(assign);
                }
                for target in &assign.targets {
                    self.visit_assign_target_subexprs(target);
                }
                self.visit_expr(&assign.value);
            }
            Stmt::AnnAssign(ann) => {
                if self.in_function {
                    self.check_assign_target_names(&ann.target);
                }
                self.visit_assign_target_subexprs(&ann.target);
                self.check_annotation(&ann.annotation);
                if let Some(value) = &ann.value {
                    self.visit_expr(value);
                }
            }
            Stmt::Expr(expr_stmt) => {
                if self.repl_input.is_none()
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
                if let Some(cause) = &raise.cause {
                    self.visit_expr(cause);
                }
            }
            Stmt::Import(import) => {
                for alias in &import.names {
                    self.check_module(&alias.name.id, alias.name.range());
                }
            }
            Stmt::ImportFrom(import) => {
                // A relative import names a module beside the file.
                if import.level == 0
                    && let Some(module) = &import.module
                {
                    self.check_module(&module.id, module.range());
                }
            }
            // Always allowed (or allowed at level 5): no checks, no
            // sub-expressions to recurse into.
            Stmt::Pass(_)
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
        // In `dict[str, int]` the tuple holds the type arguments, and in
        // `Callable[[int], int]` the list holds the parameters. Neither one is
        // a literal.
        if let Expr::Subscript(sub) = expr
            && let Expr::Tuple(args) = sub.slice.as_ref()
            && self.is_type_form(&sub.value)
        {
            self.visit_expr(&sub.value);
            for arg in &args.elts {
                match arg {
                    Expr::List(params) => params.elts.iter().for_each(|e| self.visit_expr(e)),
                    _ => self.visit_expr(arg),
                }
            }
            return;
        }

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
            Expr::If(e) if self.level < Level::Selection => self.push(
                &FORBIDDEN_SELECTION,
                e.range(),
                forbidden_msg("Conditional expression", self.level, Level::Selection),
            ),
            Expr::Lambda(e) if self.level < Level::Classes => self.push(
                &FORBIDDEN_LAMBDA,
                e.range(),
                forbidden_msg("`lambda`", self.level, Level::Classes),
            ),
            Expr::Compare(cmp) if self.level < Level::Classes && cmp.comparators().len() > 1 => {
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

/// Returns `true` if `op` is an arithmetic operator (`+`, `-`, `*`, `/`,
/// `//`, `%`, `**`), `false` otherwise. A bitwise or matrix operator is not.
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

/// Returns `true` if `decorators` contains a bare-name or attribute decorator
/// whose final identifier matches one of `names`, `false` otherwise. Matches
/// both `@override` and `@typing.override`, etc.
fn has_decorator_named(decorators: &[Decorator], names: &[&str]) -> bool {
    decorators.iter().any(|d| match &d.expression {
        Expr::Name(n) => names.contains(&n.id.as_str()),
        Expr::Attribute(a) => names.contains(&a.attr.as_str()),
        _ => false,
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
/// A name is used when the function reads it, or when a scope nested in the
/// function reads it without binding its own name of the same spelling.
fn check_unused_variables(body: &[Stmt], file: File, diagnostics: &mut Vec<Diagnostic>) {
    let mut walker = UnusedWalker::default();
    walker.visit_body(body);
    let mut reported: HashSet<&str> = HashSet::new();
    for &(name, range) in &walker.bindings {
        // An assignment to a `global` or `nonlocal` name binds a name of
        // another scope, and that scope may read it.
        if name.starts_with('_')
            || walker.uses.contains(name)
            || walker.globals.contains(name)
            || walker.nonlocals.contains(name)
            || !reported.insert(name)
        {
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

/// The names that a function with `body` binds in its own scope.
pub(crate) fn local_names(body: &[Stmt]) -> Vec<&str> {
    let mut walker = UnusedWalker::default();
    walker.visit_body(body);
    let UnusedWalker {
        locals,
        globals,
        nonlocals,
        ..
    } = walker;
    locals
        .into_iter()
        .filter(|name| !globals.contains(name) && !nonlocals.contains(name))
        .collect()
}

/// What a single scope binds and reads. A nested scope gets its own walker,
/// and only the names it reads from outside reach the walker of its parent.
#[derive(Default)]
struct UnusedWalker<'a> {
    /// The bindings the check reports when nothing reads them.
    bindings: Vec<(&'a str, TextRange)>,
    /// Every name the scope binds, reported or not. A nested scope that binds
    /// a name reads its own and not the one outside.
    locals: HashSet<&'a str>,
    /// The names the scope reads, and the ones its nested scopes read from it.
    uses: HashSet<&'a str>,
    globals: HashSet<&'a str>,
    nonlocals: HashSet<&'a str>,
    kind: ScopeKind,
    /// The names that the scopes nested in a class body read from outside.
    /// They skip the class body, whose names they do not see.
    past_class: HashSet<&'a str>,
    /// The targets of a walrus in a comprehension. They bind in the scope
    /// around the comprehension.
    escaped: Vec<(&'a str, TextRange)>,
}

#[derive(Default, PartialEq)]
enum ScopeKind {
    #[default]
    Function,
    Class,
    Comprehension,
}

impl<'a> UnusedWalker<'a> {
    fn new(kind: ScopeKind) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }

    fn bind(&mut self, name: &'a str, range: TextRange) {
        self.bindings.push((name, range));
        self.locals.insert(name);
    }

    /// Record every `Name` reachable through tuple/list/starred unpacking
    /// as a binding. Attribute / subscript targets aren't local bindings.
    fn add_target(&mut self, target: &'a Expr) {
        match target {
            Expr::Name(n) => self.bind(n.id.as_str(), n.range()),
            Expr::Tuple(t) => t.elts.iter().for_each(|v| self.add_target(v)),
            Expr::List(l) => l.elts.iter().for_each(|v| self.add_target(v)),
            Expr::Starred(s) => self.add_target(&s.value),
            _ => {}
        }
    }

    /// For an assignment target like `x[i] = 1` or `obj.attr = 1`, the
    /// outer expression isn't a binding but the inner names *are* read.
    /// Walk into Subscript/Attribute targets so those uses are recorded;
    /// recurse through tuple/list/starred wrappers to reach nested ones.
    fn visit_target_reads(&mut self, target: &'a Expr) {
        match target {
            Expr::Subscript(_) | Expr::Attribute(_) => self.visit_expr(target),
            Expr::Tuple(t) => t.elts.iter().for_each(|v| self.visit_target_reads(v)),
            Expr::List(l) => l.elts.iter().for_each(|v| self.visit_target_reads(v)),
            Expr::Starred(s) => self.visit_target_reads(&s.value),
            _ => {}
        }
    }

    /// The names the scope reads from the scopes around it.
    fn free_names(self) -> impl Iterator<Item = &'a str> {
        let Self {
            locals,
            uses,
            globals,
            nonlocals,
            past_class,
            ..
        } = self;
        uses.into_iter()
            .filter(move |n| !locals.contains(n) && !globals.contains(n))
            .chain(nonlocals)
            .chain(past_class)
    }

    fn add_nested(&mut self, mut nested: UnusedWalker<'a>) {
        for (name, range) in std::mem::take(&mut nested.escaped) {
            if self.kind == ScopeKind::Comprehension {
                self.escaped.push((name, range));
            } else {
                self.bind(name, range);
            }
        }
        // A scope nested in a class body does not see the names the class
        // binds, so its reads go past the class.
        if self.kind == ScopeKind::Class {
            self.past_class.extend(nested.free_names());
        } else {
            self.uses.extend(nested.free_names());
        }
    }

    /// The defaults and annotations of the parameters are evaluated here, and
    /// the names of the parameters are bound in `nested`.
    fn visit_parameters_of(&mut self, parameters: &'a Parameters, nested: &mut UnusedWalker<'a>) {
        for parameter in parameters {
            if let Some(default) = parameter.default() {
                self.visit_expr(default);
            }
            if let Some(annotation) = parameter.annotation() {
                self.visit_annotation(annotation);
            }
            nested.locals.insert(parameter.name().as_str());
        }
    }

    fn bind_type_params(nested: &mut UnusedWalker<'a>, type_params: Option<&'a TypeParams>) {
        for type_param in type_params.into_iter().flat_map(|t| t.iter()) {
            nested.locals.insert(type_param.name().as_str());
        }
    }

    /// The iterable of the first `for` is evaluated here, and everything else
    /// in the comprehension is evaluated in a scope of its own.
    fn visit_comprehension_scope(&mut self, generators: &'a [Comprehension], parts: &[&'a Expr]) {
        let mut nested = UnusedWalker::new(ScopeKind::Comprehension);
        for (i, generator) in generators.iter().enumerate() {
            if i == 0 {
                self.visit_expr(&generator.iter);
            } else {
                nested.visit_expr(&generator.iter);
            }
            nested.add_target(&generator.target);
            nested.visit_target_reads(&generator.target);
            for condition in &generator.ifs {
                nested.visit_expr(condition);
            }
        }
        for part in parts {
            nested.visit_expr(part);
        }
        // The targets of a comprehension are not variables the check reports.
        nested.bindings.clear();
        self.add_nested(nested);
    }
}

impl<'a> Visitor<'a> for UnusedWalker<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Assign(a) => {
                for t in &a.targets {
                    self.add_target(t);
                    // Targets like `x[i]` or `obj.attr` aren't bindings, but
                    // the inner names are reads of `x` / `i` / `obj`. Walk
                    // them so those uses are recorded.
                    self.visit_target_reads(t);
                }
                self.visit_expr(&a.value);
            }
            Stmt::AnnAssign(a) => {
                self.add_target(&a.target);
                self.visit_target_reads(&a.target);
                if let Some(v) = &a.value {
                    self.visit_expr(v);
                }
                // Skip the annotation: we don't track whether type names are
                // "used", as that's the type checker's concern.
            }
            Stmt::AugAssign(a) => {
                // `x += 1` reads then writes x. The target's `Name` carries
                // ctx=Store, so the default visitor would skip it; record
                // the read explicitly here.
                if let Expr::Name(n) = a.target.as_ref() {
                    self.uses.insert(n.id.as_str());
                    self.locals.insert(n.id.as_str());
                } else {
                    self.visit_expr(&a.target);
                }
                self.visit_expr(&a.value);
            }
            Stmt::For(f) => {
                self.add_target(&f.target);
                self.visit_target_reads(&f.target);
                self.visit_expr(&f.iter);
                self.visit_body(&f.body);
                self.visit_body(&f.orelse);
            }
            Stmt::FunctionDef(func) => {
                self.bind(func.name.as_str(), func.name.range());
                for d in &func.decorator_list {
                    self.visit_decorator(d);
                }
                let mut nested = UnusedWalker::new(ScopeKind::Function);
                Self::bind_type_params(&mut nested, func.type_params.as_deref());
                self.visit_parameters_of(&func.parameters, &mut nested);
                if let Some(returns) = &func.returns {
                    self.visit_annotation(returns);
                }
                nested.visit_body(&func.body);
                self.add_nested(nested);
            }
            Stmt::ClassDef(cls) => {
                self.bind(cls.name.as_str(), cls.name.range());
                for d in &cls.decorator_list {
                    self.visit_decorator(d);
                }
                if let Some(args) = &cls.arguments {
                    self.visit_arguments(args);
                }
                let mut nested = UnusedWalker::new(ScopeKind::Class);
                Self::bind_type_params(&mut nested, cls.type_params.as_deref());
                nested.visit_body(&cls.body);
                self.add_nested(nested);
            }
            Stmt::With(w) => {
                for item in &w.items {
                    self.visit_expr(&item.context_expr);
                    if let Some(v) = &item.optional_vars {
                        self.add_target(v);
                        self.visit_target_reads(v);
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
                        self.bind(name.as_str(), name.range());
                    }
                    self.visit_body(&h.body);
                }
                self.visit_body(&t.orelse);
                self.visit_body(&t.finalbody);
            }
            Stmt::Import(import) => {
                for alias in &import.names {
                    let name = alias.asname.as_ref().map_or_else(
                        || alias.name.split('.').next().unwrap_or_default(),
                        |asname| asname.as_str(),
                    );
                    self.locals.insert(name);
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    self.locals
                        .insert(alias.asname.as_ref().unwrap_or(&alias.name).as_str());
                }
            }
            Stmt::Global(g) => self.globals.extend(g.names.iter().map(|n| n.as_str())),
            Stmt::Nonlocal(n) => self.nonlocals.extend(n.names.iter().map(|n| n.as_str())),
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Name(n) => match n.ctx {
                ExprContext::Load => {
                    self.uses.insert(n.id.as_str());
                }
                // A store reaches here only where `add_target` is not called,
                // and `del x` makes `x` local too.
                ExprContext::Store | ExprContext::Del => {
                    self.locals.insert(n.id.as_str());
                }
                ExprContext::Invalid => {}
            },
            Expr::Named(named) => {
                // Walrus `(target := value)` binds target and reads value.
                if let (ScopeKind::Comprehension, Expr::Name(n)) = (&self.kind, &*named.target) {
                    self.escaped.push((n.id.as_str(), n.range()));
                } else {
                    self.add_target(&named.target);
                }
                self.visit_expr(&named.value);
            }
            Expr::Lambda(lambda) => {
                let mut nested = UnusedWalker::new(ScopeKind::Function);
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameters_of(parameters, &mut nested);
                }
                nested.visit_expr(&lambda.body);
                // A lambda binds only its parameters, which are not variables
                // the check reports.
                nested.bindings.clear();
                self.add_nested(nested);
            }
            Expr::ListComp(c) => self.visit_comprehension_scope(&c.generators, &[&c.elt]),
            Expr::SetComp(c) => self.visit_comprehension_scope(&c.generators, &[&c.elt]),
            Expr::Generator(c) => self.visit_comprehension_scope(&c.generators, &[&c.elt]),
            Expr::DictComp(c) => {
                let mut parts: Vec<&'a Expr> = vec![&c.value];
                parts.extend(c.key.as_deref());
                self.visit_comprehension_scope(&c.generators, &parts);
            }
            _ => walk_expr(self, expr),
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        let name = match pattern {
            Pattern::MatchAs(p) => p.name.as_ref(),
            Pattern::MatchStar(p) => p.name.as_ref(),
            Pattern::MatchMapping(p) => p.rest.as_ref(),
            _ => None,
        };
        if let Some(name) = name {
            self.locals.insert(name.as_str());
        }
        walk_pattern(self, pattern);
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

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_db::files::system_path_to_file;
    use ruff_db::system::{
        InMemorySystem, MemoryFileSystem, SystemPath, SystemPathBuf, WritableSystem,
    };

    /// Run the annotation checker on `source` and return the diagnostic IDs
    /// in the order they were emitted.
    fn check(source: &str, level: Level) -> Vec<String> {
        let cwd = SystemPathBuf::from("/");
        let system = InMemorySystem::from_memory_fs(MemoryFileSystem::with_current_directory(&cwd));
        let path = SystemPathBuf::from("/test.py");
        system
            .write_file(SystemPath::new("/test.py"), source)
            .expect("write");
        let mut db =
            crate::check::project_database(cwd.clone(), vec![cwd], vec![], system).expect("db");
        let file = system_path_to_file(&db, &path).expect("file");
        db.project().set_included_paths(&mut db, vec![path]);
        check_file_annotations(&db, file, level, None)
            .into_iter()
            .map(|d| d.id().as_str().to_string())
            .collect()
    }

    fn count(diags: &[String], id: &str) -> usize {
        diags.iter().filter(|d| d.as_str() == id).count()
    }

    #[test]
    fn a_name_with_the_reserved_prefix_is_rejected_anywhere() {
        let source = "__spython_a__ = 1\n\n\ndef __spython_f__(__spython_p: int) -> int:\n    return __spython_p\n\n\nclass C:\n    pass\n\n\nc = C()\nc.__spython_x = 1\ns = f\"{__spython_a__}\"\n_spython_y = 1\n__spython = 1\n";
        let diags = check(source, Level::Full);
        assert_eq!(count(&diags, "reserved-name"), 6, "{diags:?}");
    }

    #[test]
    fn the_guard_of_a_case_is_a_condition() {
        let source = "def f(x: int) -> int:\n    match x:\n        case 1 if x:\n            return 1\n        case _:\n            return 2\n";
        let diags = check(source, Level::UserTypes);
        assert_eq!(count(&diags, "non-boolean-condition"), 1, "{diags:?}");
        let diags = check(&source.replace("if x:", "if x > 0:"), Level::UserTypes);
        assert_eq!(count(&diags, "non-boolean-condition"), 0, "{diags:?}");
    }

    #[test]
    fn the_conditional_expression_waits_for_the_level_of_if() {
        let source = "def f(x: int) -> int:\n    return 1 if x > 0 else 2\n";
        let diags = check(source, Level::Functions);
        assert_eq!(count(&diags, "forbidden-selection"), 1, "{diags:?}");
        let diags = check(source, Level::Selection);
        assert_eq!(count(&diags, "forbidden-selection"), 0, "{diags:?}");
    }

    #[test]
    fn an_import_of_a_module_the_interpreter_lacks_is_an_error() {
        for source in [
            "import re\n",
            "import os.path\n",
            "from collections.abc import Sequence\n",
            "from __future__ import annotations\n",
            "def f() -> None:\n    import sys\n",
        ] {
            let diags = check(source, Level::Full);
            assert_eq!(count(&diags, "unavailable-module"), 1, "{source}");
        }
    }

    #[test]
    fn an_import_of_a_module_the_interpreter_has_is_not_an_error() {
        for source in [
            "import math\n",
            "from dataclasses import dataclass\n",
            "from enum import Enum\n",
            "from typing import Any\n",
            "import nosuch\n",
            "from . import sibling\n",
        ] {
            let diags = check(source, Level::Full);
            assert_eq!(count(&diags, "unavailable-module"), 0, "{source}");
        }
    }

    // -- The parameter defaults of a def are visited -------------------------

    #[test]
    fn parameter_defaults_are_visited() {
        let diags = check(
            "def f(x: str = f\"a\", *, y: bool = (1 == True)) -> str:\n    return x\n",
            Level::Full,
        );
        assert_eq!(
            count(&diags, "f-string-missing-placeholders"),
            1,
            "{diags:?}"
        );
        assert_eq!(count(&diags, "true-false-comparison"), 1, "{diags:?}");
    }

    // -- Stmt::Raise.cause is visited ----------------------------------------

    #[test]
    fn raise_cause_is_visited() {
        // At level 3 a lambda is forbidden, and this one hides in the cause of
        // the raise.
        let diags = check(
            "def f() -> None:\n    raise ValueError from (lambda: 0)()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    // -- Stmt::Match patterns are visited ------------------------------------

    #[test]
    fn match_pattern_value_is_visited() {
        // F541 does not depend on the level, and it finds the value of a case
        // only if the checker visits the pattern.
        let diags = check(
            "def f(x: object) -> None:\n    match x:\n        case f\"\":\n            pass\n",
            Level::Full,
        );
        assert!(
            diags.contains(&"f-string-missing-placeholders".to_string()),
            "{diags:?}"
        );
    }

    // -- Stmt::With.optional_vars is name-checked and visited ----------------

    #[test]
    fn with_optional_vars_name_check() {
        // N806: bound name in `as Bar` should be lowercase.
        let diags = check(
            "def f() -> None:\n    with open(\"x\") as Bar:\n        pass\n",
            Level::Full,
        );
        assert!(
            diags.contains(&"non-lowercase-variable-in-function".to_string()),
            "{diags:?}"
        );
    }

    // -- Stmt::Try handler types are visited ----------------------------------

    #[test]
    fn except_type_is_visited() {
        let diags = check(
            "def f(x: str) -> int:\n    try:\n        return int(x)\n    except (ValueError if x == None else TypeError):\n        return 0\n",
            Level::Full,
        );
        assert!(diags.contains(&"none-comparison".to_string()), "{diags:?}");
    }

    #[test]
    fn except_name_check() {
        let diags = check(
            "def f(x: str) -> int:\n    try:\n        return int(x)\n    except ValueError as Err:\n        return 0\n",
            Level::Full,
        );
        assert!(
            diags.contains(&"non-lowercase-variable-in-function".to_string()),
            "{diags:?}"
        );
    }

    // -- Stmt::ClassDef base classes / kwargs are visited --------------------

    #[test]
    fn class_base_argument_is_visited() {
        // metaclass=lambda forbidden at level 3 (lambda < Classes).
        let diags = check(
            "class Foo(metaclass=(lambda: 0)):\n    x: int = 0\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    #[test]
    fn class_positional_base_is_visited() {
        // f-string-missing-placeholders is independent of teaching level.
        let diags = check("class Foo(type(f\"\")):\n    x: int = 0\n", Level::Full);
        assert!(
            diags.contains(&"f-string-missing-placeholders".to_string()),
            "{diags:?}"
        );
    }

    // -- Stmt::AugAssign target sub-expressions are visited ------------------

    #[test]
    fn aug_assign_subscript_slice_is_visited() {
        // In x[lambda: 0] += 1, the slice contains a forbidden lambda at level 3.
        // (Aug assign and subscript indexing are both allowed at level 3.)
        let diags = check(
            "def f(x: list[int]) -> None:\n    x[(lambda: 0)()] += 1\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    // -- Stmt::Assign target sub-expressions are visited ---------------------

    #[test]
    fn assign_subscript_slice_is_visited() {
        let diags = check(
            "def f(d: dict[int, int]) -> None:\n    d[(lambda: 0)()] = 1\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    #[test]
    fn assign_attribute_value_is_visited() {
        // f-string-missing-placeholders fires on f"" inside an attribute target.
        let diags = check(
            "class Foo:\n    x: int = 0\n\ndef f(o: Foo) -> None:\n    (f\"\" or o).x = 1\n",
            Level::Full,
        );
        assert!(
            diags.contains(&"f-string-missing-placeholders".to_string()),
            "{diags:?}"
        );
    }

    #[test]
    fn type_arguments_are_not_a_literal() {
        let code = "from dataclasses import dataclass\n\
from typing import Callable\n\n\
@dataclass\nclass P[T, U]:\n    a: T\n    b: U\n\n\
p = P[int, str](1, \"a\")\n\
D = dict[str, int]\n\
C = Callable[[int], int]\n";
        let diags = check(code, Level::UserTypes);
        assert_eq!(
            count(&diags, "forbidden-collection-literal"),
            0,
            "{diags:?}"
        );
    }

    #[test]
    fn a_tuple_index_is_still_a_literal() {
        let code = "def f(d: dict[tuple[int, int], int]) -> int:\n    return d[1, 2]\n";
        let diags = check(code, Level::UserTypes);
        assert_eq!(
            count(&diags, "forbidden-collection-literal"),
            1,
            "{diags:?}"
        );
    }

    #[test]
    fn assign_tuple_unpacking_is_not_flagged_as_literal() {
        // Tuple unpacking on the LHS must NOT trigger
        // FORBIDDEN_COLLECTION_LITERAL even at level 0.
        let diags = check(
            "def f() -> tuple[int, int]:\n    return (1, 2)\n\ndef g() -> None:\n    a, b = f()\n",
            Level::Functions,
        );
        // The Tuple in the RHS of `return (1, 2)` does fire (it's a literal),
        // but the LHS `a, b` must not.
        assert_eq!(
            count(&diags, "forbidden-collection-literal"),
            1,
            "{diags:?}"
        );
    }

    // -- Stmt::AnnAssign target sub-expressions are visited ------------------

    #[test]
    fn ann_assign_subscript_slice_is_visited() {
        let diags = check(
            "def f(d: dict[int, int]) -> None:\n    d[(lambda: 0)()]: int = 1\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    // -- Stmt::For target sub-expressions are visited -----------------------

    #[test]
    fn for_target_subscript_slice_is_visited() {
        let diags = check(
            "def f(arr: list[int]) -> None:\n    for arr[(lambda: 0)()] in [1, 2]:\n        pass\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "forbidden-lambda"), 1, "{diags:?}");
    }

    // -- UnusedWalker: subscript / attribute target reads are recorded -------

    #[test]
    fn unused_walker_records_subscript_target_reads() {
        // `a` is only read by `a[0] = 1`. The Subscript target's value is a
        // load of `a`.
        let diags = check(
            "def f(arr: list[int]) -> None:\n    a: list[int] = arr\n    a[0] = 1\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_attribute_target_reads() {
        let diags = check(
            "class P:\n    x: int = 0\n\ndef f(p: P) -> None:\n    o: P = p\n    o.x = 1\n",
            Level::UserTypes,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_the_reads_of_a_closure() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    def g() -> int:\n        def h() -> int:\n            return n\n        return h()\n    return g()\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_in_a_default() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    def h(x: int = n) -> int:\n        return x\n    return h()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_in_a_decorator() {
        let diags = check(
            "def f(d: int) -> None:\n    n: int = d\n    @n\n    def h() -> None:\n        pass\n    h()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_a_parameter_hides() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    def g(n: int) -> int:\n        return n\n    return g(2)\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_a_nested_assignment_hides() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    def g() -> int:\n        n: int = 2\n        return n\n    return g()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_a_nested_import_hides() {
        let diags = check(
            "def f() -> float:\n    pi: float = 1.0\n    def g() -> float:\n        from math import pi\n        return pi\n    return g()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_of_a_method_past_the_class() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    class C:\n        n: int = 2\n        def m(self) -> int:\n            return n\n    return C().m()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_the_class_body_hides() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    class C:\n        n: int = 2\n        k: int = n\n    return C.k\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_of_the_class_body() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    class C:\n        k: int = n\n    return C.k\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_in_a_lambda() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    g = lambda: n\n    return g()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_a_lambda_parameter_hides() {
        let diags = check(
            "def f() -> int:\n    n: int = 1\n    g = lambda n: n\n    return g(2)\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_a_name_a_comprehension_target_hides() {
        let diags = check(
            "def f(xs: list[int]) -> list[int]:\n    n: int = 1\n    return [n for n in xs]\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_read_in_a_comprehension() {
        let diags = check(
            "def f(xs: list[int]) -> list[int]:\n    n: int = 1\n    return [x + n for x in xs]\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_binds_a_comprehension_walrus_in_the_function() {
        let diags = check(
            "def f(xs: list[int]) -> int:\n    [(last := x) for x in xs]\n    return last\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_flags_an_unread_comprehension_walrus() {
        let diags = check(
            "def f(xs: list[int]) -> None:\n    [(last := x) for x in xs]\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    #[test]
    fn unused_walker_records_a_nonlocal_rebinding() {
        let diags = check(
            "def f() -> None:\n    n: int = 1\n    def g() -> None:\n        nonlocal n\n        n = 2\n    g()\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_skips_an_assignment_to_a_global() {
        let diags = check(
            "def f() -> None:\n    global n\n    n = 2\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "unused-variable"), 0, "{diags:?}");
    }

    #[test]
    fn unused_walker_still_flags_truly_unused() {
        // Sanity check: a local that's never read is still flagged.
        let diags = check("def f() -> None:\n    a: int = 1\n", Level::Functions);
        assert_eq!(count(&diags, "unused-variable"), 1, "{diags:?}");
    }

    // -- redefined-function-or-class -----------------------------------------

    #[test]
    fn function_redefined_at_module_level_is_flagged() {
        let diags = check(
            "def f() -> int:\n    return 1\n\ndef f() -> int:\n    return 2\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 1, "{diags:?}");
    }

    #[test]
    fn class_redefined_at_module_level_is_flagged() {
        let diags = check(
            "class Foo:\n    x: int = 0\n\nclass Foo:\n    x: int = 0\n",
            Level::UserTypes,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 1, "{diags:?}");
    }

    #[test]
    fn function_then_class_with_same_name_is_flagged() {
        let diags = check(
            "def Foo() -> int:\n    return 1\n\nclass Foo:\n    x: int = 0\n",
            Level::UserTypes,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 1, "{diags:?}");
    }

    #[test]
    fn methods_with_same_name_in_class_body_is_flagged() {
        // Use Repetition (3). Methods in a class are forbidden there
        // (FORBIDDEN_CLASS_METHOD also fires) but the redefinition check
        // still applies because we walk the class body regardless. At
        // level 4+ redefinition is intentionally allowed.
        let diags = check(
            "class Foo:\n    def m(self) -> int:\n        return 1\n    def m(self) -> int:\n        return 2\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 1, "{diags:?}");
    }

    #[test]
    fn redefinition_is_not_flagged_at_classes_level() {
        let diags = check(
            "def f() -> int:\n    return 1\n\ndef f() -> int:\n    return 2\n",
            Level::Classes,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 0, "{diags:?}");
    }

    #[test]
    fn redefinition_is_not_flagged_at_full_level() {
        let diags = check(
            "class Foo:\n    x: int = 0\n\nclass Foo:\n    x: int = 0\n",
            Level::Full,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 0, "{diags:?}");
    }

    // -- missing-type-parameter (bare `list` annotation) ---------------------

    #[test]
    fn bare_list_in_param_annotation_is_flagged() {
        let diags = check(
            "def f(x: list) -> None:\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn parameterized_list_in_param_annotation_is_not_flagged() {
        let diags = check(
            "def f(x: list[int]) -> None:\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 0, "{diags:?}");
    }

    #[test]
    fn bare_list_in_return_annotation_is_flagged() {
        let diags = check("def f() -> list:\n    return [1]\n", Level::Repetition);
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn bare_list_in_ann_assign_is_flagged() {
        let diags = check(
            "def f() -> None:\n    x: list = [1]\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn bare_list_in_class_attribute_is_flagged() {
        let diags = check("class Foo:\n    items: list = [1]\n", Level::Repetition);
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn bare_list_nested_in_dict_param_is_flagged() {
        let diags = check(
            "def f(x: dict[str, list]) -> None:\n    return None\n",
            Level::Full,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn bare_list_in_union_is_flagged() {
        let diags = check(
            "def f(x: int | list) -> None:\n    return None\n",
            Level::Full,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 1, "{diags:?}");
    }

    #[test]
    fn nested_parameterized_list_is_not_flagged() {
        let diags = check(
            "def f(x: list[list[int]]) -> None:\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "missing-type-parameter"), 0, "{diags:?}");
    }

    // -- local-unknown-type --------------------------------------------------

    #[test]
    fn bare_assign_with_unknown_rhs_is_flagged() {
        // `undefined_name` is unresolved, so the RHS and therefore `x` have
        // type `Unknown`.
        let diags = check(
            "def f() -> None:\n    x = undefined_name\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "local-unknown-type"), 1, "{diags:?}");
    }

    #[test]
    fn annotated_assign_with_unknown_rhs_is_not_flagged() {
        // The annotation provides the type even if the RHS is Unknown.
        let diags = check(
            "def f() -> None:\n    x: int = undefined_name\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "local-unknown-type"), 0, "{diags:?}");
    }

    #[test]
    fn bare_assign_with_known_rhs_is_not_flagged() {
        let diags = check(
            "def f() -> None:\n    x = 5\n    return None\n",
            Level::Repetition,
        );
        assert_eq!(count(&diags, "local-unknown-type"), 0, "{diags:?}");
    }

    #[test]
    fn module_level_assign_with_unknown_rhs_is_not_flagged() {
        // "em locais" means that only function-scoped assignments are flagged.
        let diags = check("x = undefined_name\n", Level::Repetition);
        assert_eq!(count(&diags, "local-unknown-type"), 0, "{diags:?}");
    }

    #[test]
    fn tuple_unpack_with_unknown_rhs_is_flagged() {
        let diags = check(
            "def f() -> None:\n    a, b = undefined_name\n    return None\n",
            Level::Full,
        );
        assert!(count(&diags, "local-unknown-type") >= 1, "{diags:?}");
    }

    #[test]
    fn unrelated_name_called_list_is_not_flagged_outside_annotation() {
        // `list = ...` is a regular assignment, not a type annotation. We
        // only check annotation positions, so this should not fire.
        let diags = check("def f() -> int:\n    return 1\n", Level::Repetition);
        assert_eq!(count(&diags, "missing-type-parameter"), 0, "{diags:?}");
    }

    #[test]
    fn nested_function_redefined_is_flagged() {
        let diags = check(
            "def outer() -> int:\n    def inner() -> int:\n        return 1\n    def inner() -> int:\n        return 2\n    return inner()\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 1, "{diags:?}");
    }

    #[test]
    fn overload_decorated_functions_are_not_flagged() {
        // Lowest level where the redefinition check runs, so the test
        // exercises the @overload exemption rather than the level guard.
        let diags = check(
            "from typing import overload\n\n@overload\ndef f(x: int) -> int: ...\n@overload\ndef f(x: str) -> str: ...\ndef f(x: object) -> object:\n    return x\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 0, "{diags:?}");
    }

    #[test]
    fn same_name_in_disjoint_if_branches_is_not_flagged() {
        let diags = check(
            "def cond() -> bool:\n    return True\n\nif cond():\n    def f() -> int:\n        return 1\nelse:\n    def f() -> int:\n        return 2\n",
            Level::Selection,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 0, "{diags:?}");
    }

    #[test]
    fn same_name_in_different_scopes_is_not_flagged() {
        // Module-level `helper` and an inner `helper` inside `outer` are in
        // different scopes and must not be flagged.
        let diags = check(
            "def helper() -> int:\n    return 1\n\ndef outer() -> int:\n    def helper() -> int:\n        return 2\n    return helper()\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 0, "{diags:?}");
    }

    #[test]
    fn triple_redefinition_emits_two_diagnostics() {
        let diags = check(
            "def f() -> int:\n    return 1\n\ndef f() -> int:\n    return 2\n\ndef f() -> int:\n    return 3\n",
            Level::Functions,
        );
        assert_eq!(count(&diags, "redefined-function-or-class"), 2, "{diags:?}");
    }
}
