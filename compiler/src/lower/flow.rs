//! What the lowering reads from a body before it lowers it: the names that
//! the body binds, their order in the code, and whether the body always
//! returns or breaks.

use std::collections::HashSet;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr};

/// Adds to `names` each name of `body` in the order of the code that
/// CPython makes for it, which gives the order of the locals of a frame.
/// The value of an assignment comes before its targets.
pub(super) fn code_order<'a>(body: &'a [ast::Stmt], names: &mut Vec<&'a str>) {
    struct Names<'a, 'b>(&'b mut Vec<&'a str>);
    impl<'a> Visitor<'a> for Names<'a, '_> {
        fn visit_expr(&mut self, expr: &'a ast::Expr) {
            if let ast::Expr::Name(name) = expr {
                self.0.push(name.id.as_str());
            }
            walk_expr(self, expr);
        }
    }
    let visit = |expr: &'a ast::Expr, names: &mut Vec<&'a str>| {
        Names(names).visit_expr(expr);
    };
    for stmt in body {
        match stmt {
            ast::Stmt::Expr(e) => visit(&e.value, names),
            ast::Stmt::Assign(a) => {
                visit(&a.value, names);
                for target in &a.targets {
                    visit(target, names);
                }
            }
            // CPython makes no code for a declaration without a value.
            ast::Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    visit(value, names);
                    visit(&a.target, names);
                }
            }
            ast::Stmt::AugAssign(a) => {
                visit(&a.target, names);
                visit(&a.value, names);
            }
            ast::Stmt::If(i) => {
                visit(&i.test, names);
                code_order(&i.body, names);
                for clause in &i.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        visit(test, names);
                    }
                    code_order(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                visit(&w.test, names);
                code_order(&w.body, names);
                code_order(&w.orelse, names);
            }
            ast::Stmt::Return(r) => {
                if let Some(value) = &r.value {
                    visit(value, names);
                }
            }
            _ => {}
        }
    }
}

/// The targets of an assignment, with the value.
pub(super) type Assigned<'a> = (&'a [ast::Expr], &'a ast::Expr);

/// Adds to `assigned` each assignment of a value in `body`, and to `defs`
/// each `def`, out of the `if`s and the `while`s too.
pub(super) fn assignments<'a>(
    body: &'a [ast::Stmt],
    assigned: &mut Vec<Assigned<'a>>,
    defs: &mut Vec<&'a ast::StmtFunctionDef>,
) {
    for stmt in body {
        match stmt {
            ast::Stmt::If(i) => {
                assignments(&i.body, assigned, defs);
                for clause in &i.elif_else_clauses {
                    assignments(&clause.body, assigned, defs);
                }
            }
            ast::Stmt::While(w) => {
                assignments(&w.body, assigned, defs);
                assignments(&w.orelse, assigned, defs);
            }
            ast::Stmt::FunctionDef(def) => defs.push(def),
            ast::Stmt::Assign(a) => assigned.push((&a.targets, &a.value)),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => assigned.push((std::slice::from_ref(target), value)),
            _ => {}
        }
    }
}

/// Adds to `names` each name of a `global` statement in `body`.
pub(super) fn global_names(body: &[ast::Stmt], names: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            ast::Stmt::Global(g) => names.extend(g.names.iter().map(|name| name.id.to_string())),
            ast::Stmt::If(i) => {
                global_names(&i.body, names);
                for clause in &i.elif_else_clauses {
                    global_names(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                global_names(&w.body, names);
                global_names(&w.orelse, names);
            }
            _ => {}
        }
    }
}

/// Adds to `names` each name that `body` assigns. Any other way to bind a
/// name, as a `for` or a `:=`, makes the lowering of the function fail.
pub(super) fn local_names(body: &[ast::Stmt], names: &mut HashSet<String>) {
    fn add(target: &ast::Expr, names: &mut HashSet<String>) {
        if let ast::Expr::Name(name) = target {
            names.insert(name.id.to_string());
        }
    }
    for stmt in body {
        match stmt {
            ast::Stmt::Assign(a) => {
                for target in &a.targets {
                    add(target, names);
                }
            }
            ast::Stmt::AnnAssign(a) => add(&a.target, names),
            ast::Stmt::AugAssign(a) => add(&a.target, names),
            ast::Stmt::If(i) => {
                local_names(&i.body, names);
                for clause in &i.elif_else_clauses {
                    local_names(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                local_names(&w.body, names);
                local_names(&w.orelse, names);
            }
            _ => {}
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
