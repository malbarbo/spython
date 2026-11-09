//! What the lowering reads from a body before it lowers it: the names that
//! the body binds, their order in the code, and whether the body always
//! returns or breaks.

use std::collections::HashSet;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr};

/// The name that `alias` binds, in a `from` import or not. `import a.b`
/// binds `a`.
pub(super) fn imported_name(alias: &ast::Alias, from: bool) -> &str {
    match &alias.asname {
        Some(asname) => asname.as_str(),
        None if from => alias.name.as_str(),
        None => alias.name.split('.').next().unwrap_or_default(),
    }
}

/// The targets of an assignment, with the value.
pub(super) type Assigned<'a> = (&'a [ast::Expr], &'a ast::Expr);

/// What a body binds, out of its `if`s and its `while`s too.
#[derive(Default)]
pub(super) struct Bindings<'a> {
    /// Each assignment of a value.
    pub assignments: Vec<Assigned<'a>>,
    pub defs: Vec<&'a ast::StmtFunctionDef>,
    /// Each name that an import binds, with the alias that binds it.
    pub imports: Vec<(&'a str, &'a ast::Alias)>,
    /// The names of the `global` statements.
    pub globals: HashSet<&'a str>,
    /// Each name that the body assigns or imports. Any other way to bind a
    /// name, as a `for` or a `:=`, makes the lowering of the function fail.
    pub assigned: HashSet<&'a str>,
    /// Each name of the body in the order of the code that CPython makes
    /// for it, which gives the order of the locals of a frame. The value of
    /// an assignment comes before its targets.
    pub order: Vec<&'a str>,
}

impl<'a> Bindings<'a> {
    pub fn of(body: &'a [ast::Stmt]) -> Self {
        let mut bindings = Bindings::default();
        bindings.add(body);
        bindings
    }

    fn add(&mut self, body: &'a [ast::Stmt]) {
        for stmt in body {
            match stmt {
                ast::Stmt::Expr(e) => self.visit(&e.value),
                ast::Stmt::Assign(a) => {
                    self.visit(&a.value);
                    for target in &a.targets {
                        self.visit(target);
                        self.assign(target);
                    }
                    self.assignments.push((&a.targets, &a.value));
                }
                ast::Stmt::AnnAssign(a) => {
                    // CPython makes no code for a declaration without a value.
                    if let Some(value) = &a.value {
                        self.visit(value);
                        self.visit(&a.target);
                        self.assignments
                            .push((std::slice::from_ref(&a.target), value));
                    }
                    self.assign(&a.target);
                }
                ast::Stmt::AugAssign(a) => {
                    self.visit(&a.target);
                    self.visit(&a.value);
                    self.assign(&a.target);
                }
                ast::Stmt::If(i) => {
                    self.visit(&i.test);
                    self.add(&i.body);
                    for clause in &i.elif_else_clauses {
                        if let Some(test) = &clause.test {
                            self.visit(test);
                        }
                        self.add(&clause.body);
                    }
                }
                ast::Stmt::While(w) => {
                    self.visit(&w.test);
                    self.add(&w.body);
                    self.add(&w.orelse);
                }
                ast::Stmt::Return(r) => {
                    if let Some(value) = &r.value {
                        self.visit(value);
                    }
                }
                ast::Stmt::Global(g) => self
                    .globals
                    .extend(g.names.iter().map(|name| name.id.as_str())),
                ast::Stmt::FunctionDef(def) => self.defs.push(def),
                ast::Stmt::Import(i) => {
                    for alias in &i.names {
                        self.import(imported_name(alias, false), alias);
                    }
                }
                ast::Stmt::ImportFrom(i) => {
                    for alias in &i.names {
                        self.import(imported_name(alias, true), alias);
                    }
                }
                _ => {}
            }
        }
    }

    /// Adds each name of `expr` to the order.
    fn visit(&mut self, expr: &'a ast::Expr) {
        struct Names<'a, 'b>(&'b mut Vec<&'a str>);
        impl<'a> Visitor<'a> for Names<'a, '_> {
            fn visit_expr(&mut self, expr: &'a ast::Expr) {
                if let ast::Expr::Name(name) = expr {
                    self.0.push(name.id.as_str());
                }
                walk_expr(self, expr);
            }
        }
        Names(&mut self.order).visit_expr(expr);
    }

    fn import(&mut self, name: &'a str, alias: &'a ast::Alias) {
        self.imports.push((name, alias));
        self.assigned.insert(name);
        self.order.push(name);
    }

    fn assign(&mut self, target: &'a ast::Expr) {
        if let ast::Expr::Name(name) = target {
            self.assigned.insert(name.id.as_str());
        }
    }
}

/// Returns `true` if `body` never reaches its end, `false` when it may.
/// The answer is `false` in a case this check cannot decide.
pub(super) fn always_returns(body: &[ast::Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        ast::Stmt::Return(_) => true,
        ast::Stmt::If(i) => {
            always_returns(&i.body)
                && i.elif_else_clauses
                    .last()
                    .is_some_and(|clause| clause.test.is_none())
                && i.elif_else_clauses
                    .iter()
                    .all(|clause| always_returns(&clause.body))
        }
        ast::Stmt::While(w) => {
            matches!(&*w.test, ast::Expr::BooleanLiteral(b) if b.value) && !breaks(&w.body)
        }
        _ => false,
    })
}

/// Returns `true` if `body` has a `break` of the loop whose body it is,
/// `false` otherwise.
fn breaks(body: &[ast::Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        ast::Stmt::Break(_) => true,
        ast::Stmt::If(i) => {
            breaks(&i.body)
                || i.elif_else_clauses
                    .iter()
                    .any(|clause| breaks(&clause.body))
        }
        // A `break` in the body of an inner loop leaves that loop, and one
        // in its `else` leaves this one.
        ast::Stmt::While(w) => breaks(&w.orelse),
        _ => false,
    })
}
