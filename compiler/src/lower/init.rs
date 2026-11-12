//! The proof that an `__init__` assigns each field of its object before it
//! reads the field and before `self` goes anywhere else, as the definite
//! assignment of Java does. A field without a value raises
//! `AttributeError` in CPython, and the struct has no such state.

use std::cell::Cell;
use std::collections::HashSet;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_text_size::{Ranged, TextRange};

use super::unsupported;
use crate::Error;

/// What `super().__init__(...)` does in an `__init__`, which calls the
/// `__init__` of the base, or the one of `object` without a base.
#[derive(Clone, Copy)]
pub(super) struct Base<'f> {
    /// The fields that the `__init__` of the base assigns, which are the
    /// first fields of the subclass.
    pub fields: &'f [(&'f str, TextRange)],
    /// `true` when the `__init__` of the base lets `self` escape, where a
    /// method of the subclass may read a field of the subclass.
    pub escapes: bool,
}

/// Checks that `init` assigns each field of `fields`, each with the node
/// of its annotation, in every path, before a read of the field and
/// before `self` escapes. `self` escapes in an argument, in an assignment
/// to another variable or field, in `super()`, and in any use other than
/// the read or the assignment of a field. `super().__init__(...)` as a
/// statement assigns the fields of `base`, and lets `self` escape when the
/// `__init__` of the base does. Returns `true` if `self` may escape,
/// `false` otherwise.
pub(super) fn check_init(
    init: &ast::StmtFunctionDef,
    fields: &[(&str, TextRange)],
    base: Base,
) -> Result<bool, Error> {
    let receiver = init
        .parameters
        .args
        .first()
        .expect("a method with self")
        .name()
        .as_str();
    let check = Check {
        receiver,
        fields,
        base,
        escaped: Cell::new(false),
    };
    let after = check.body(&init.body, Some(HashSet::new()))?;
    check.all_assigned(after.as_ref())?;
    Ok(check.escaped.get())
}

struct Check<'a> {
    /// The name of the first parameter of `__init__`.
    receiver: &'a str,
    fields: &'a [(&'a str, TextRange)],
    base: Base<'a>,
    /// Whether `self` escaped somewhere.
    escaped: Cell<bool>,
}

/// The fields that have a value at a point of the code, and `None` at a
/// point that the code never reaches.
type Assigned<'a> = Option<HashSet<&'a str>>;

impl<'a> Check<'a> {
    fn body(&self, body: &'a [ast::Stmt], mut state: Assigned<'a>) -> Result<Assigned<'a>, Error> {
        for stmt in body {
            state = self.stmt(stmt, state)?;
        }
        Ok(state)
    }

    fn stmt(&self, stmt: &'a ast::Stmt, state: Assigned<'a>) -> Result<Assigned<'a>, Error> {
        let Some(mut assigned) = state else {
            return Ok(None);
        };
        match stmt {
            ast::Stmt::Expr(e) => match super_init(&e.value) {
                Some(call) => {
                    self.reads_all(call, &assigned)?;
                    if self.base.escapes {
                        let own = self
                            .fields
                            .get(self.base.fields.len()..)
                            .unwrap_or_default();
                        self.escapes_with(call.range(), &assigned, own)?;
                    }
                    assigned.extend(self.base.fields.iter().map(|(name, _)| *name));
                }
                None => self.reads(&e.value, &assigned)?,
            },
            ast::Stmt::Assign(a) => {
                self.reads(&a.value, &assigned)?;
                for target in &a.targets {
                    if let Some(field) = self.field_of(target) {
                        assigned.insert(field);
                    } else {
                        self.reads(target, &assigned)?;
                    }
                }
            }
            // The lowering takes a name alone as the target, and an
            // annotation without a value reads nothing.
            ast::Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    self.reads(value, &assigned)?;
                }
            }
            // The update reads the field first.
            ast::Stmt::AugAssign(a) => {
                self.reads(&a.target, &assigned)?;
                self.reads(&a.value, &assigned)?;
            }
            ast::Stmt::If(i) => {
                self.reads(&i.test, &assigned)?;
                let mut after = self.body(&i.body, Some(assigned.clone()))?;
                let mut covered = false;
                for clause in &i.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        self.reads(test, &assigned)?;
                    } else {
                        covered = true;
                    }
                    let branch = self.body(&clause.body, Some(assigned.clone()))?;
                    after = meet(after, branch);
                }
                if !covered {
                    after = meet(after, Some(assigned));
                }
                return Ok(after);
            }
            // A loop may run its body no time, and a `break` may leave it
            // before the `else`.
            ast::Stmt::While(w) => {
                self.reads(&w.test, &assigned)?;
                self.body(&w.body, Some(assigned.clone()))?;
                self.body(&w.orelse, Some(assigned.clone()))?;
            }
            ast::Stmt::For(f) => {
                self.reads(&f.iter, &assigned)?;
                self.reads(&f.target, &assigned)?;
                self.body(&f.body, Some(assigned.clone()))?;
                self.body(&f.orelse, Some(assigned.clone()))?;
            }
            ast::Stmt::Return(r) => {
                if let Some(value) = &r.value {
                    self.reads(value, &assigned)?;
                }
                self.all_assigned(Some(&assigned))?;
                return Ok(None);
            }
            ast::Stmt::Raise(r) => {
                for value in r.exc.iter().chain(&r.cause) {
                    self.reads(value, &assigned)?;
                }
                return Ok(None);
            }
            ast::Stmt::Break(_) | ast::Stmt::Continue(_) => return Ok(None),
            // Any other statement may run its parts in any order, so `self`
            // escapes in each use.
            stmt => {
                let mut uses = Uses {
                    receiver: self.receiver,
                    found: None,
                };
                uses.visit_stmt(stmt);
                if let Some(range) = uses.found {
                    self.escapes(range, &assigned)?;
                }
            }
        }
        Ok(Some(assigned))
    }

    /// The field that `target` names, when it is a field of `self`.
    fn field_of(&self, target: &'a ast::Expr) -> Option<&'a str> {
        let ast::Expr::Attribute(attr) = target else {
            return None;
        };
        let is_self =
            matches!(&*attr.value, ast::Expr::Name(name) if name.id.as_str() == self.receiver);
        let field = attr.attr.as_str();
        (is_self && self.fields.iter().any(|(name, _)| *name == field)).then_some(field)
    }

    /// Checks each use of `self` in `expr`, with the fields in `assigned`.
    fn reads(&self, expr: &'a ast::Expr, assigned: &HashSet<&'a str>) -> Result<(), Error> {
        let mut reads = Reads {
            check: self,
            assigned,
            error: None,
        };
        reads.visit_expr(expr);
        reads.error.map_or(Ok(()), Err)
    }

    /// Checks each argument of `call` with the fields in `assigned`.
    fn reads_all(&self, call: &'a ast::ExprCall, assigned: &HashSet<&'a str>) -> Result<(), Error> {
        for arg in &call.arguments.args {
            self.reads(arg, assigned)?;
        }
        for keyword in &call.arguments.keywords {
            self.reads(&keyword.value, assigned)?;
        }
        Ok(())
    }

    /// Checks that `self` may escape at `range`, with the fields in
    /// `assigned`.
    fn escapes(&self, range: TextRange, assigned: &HashSet<&str>) -> Result<(), Error> {
        self.escapes_with(range, assigned, self.fields)
    }

    /// Checks that `self` may escape at `range` where a method may read
    /// `fields`, with the fields in `assigned`.
    fn escapes_with(
        &self,
        range: TextRange,
        assigned: &HashSet<&str>,
        fields: &[(&str, TextRange)],
    ) -> Result<(), Error> {
        self.escaped.set(true);
        if fields.iter().all(|(name, _)| assigned.contains(name)) {
            Ok(())
        } else {
            Err(unsupported(
                &range,
                "self before __init__ assigns each field",
            ))
        }
    }

    /// Checks that `assigned` has each field, where `__init__` returns.
    fn all_assigned(&self, assigned: Option<&HashSet<&str>>) -> Result<(), Error> {
        let Some(assigned) = assigned else {
            return Ok(());
        };
        match self
            .fields
            .iter()
            .find(|(name, _)| !assigned.contains(name))
        {
            Some((_, range)) => Err(unsupported(range, "a field that __init__ may not assign")),
            None => Ok(()),
        }
    }
}

/// The fields that have a value after two paths that join.
fn meet<'a>(a: Assigned<'a>, b: Assigned<'a>) -> Assigned<'a> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.intersection(&b).copied().collect()),
        (a, None) => a,
        (None, b) => b,
    }
}

/// The visitor of the uses of `self` in an expression.
struct Reads<'c, 'a> {
    check: &'c Check<'a>,
    assigned: &'c HashSet<&'a str>,
    error: Option<Error>,
}

impl<'a> Visitor<'a> for Reads<'_, 'a> {
    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        if self.error.is_some() {
            return;
        }
        match expr {
            ast::Expr::Attribute(attr) if self.check.field_of(expr).is_some() => {
                let field = attr.attr.as_str();
                if !self.assigned.contains(field) {
                    self.error = Some(unsupported(
                        attr,
                        "a field that __init__ may read before it assigns it",
                    ));
                }
            }
            // `super()` reads `self` without its name.
            ast::Expr::Call(call) if is_super(call) => {
                self.error = self.check.escapes(call.range(), self.assigned).err();
            }
            ast::Expr::Name(name) if name.id.as_str() == self.check.receiver => {
                let checked = if name.ctx.is_load() {
                    self.check.escapes(name.range(), self.assigned)
                } else {
                    Err(unsupported(name, "an assignment to self in __init__"))
                };
                self.error = checked.err();
            }
            // The body of a `lambda` runs later, when its function may read
            // any field.
            ast::Expr::Lambda(_) => {
                let mut uses = Uses {
                    receiver: self.check.receiver,
                    found: None,
                };
                uses.visit_expr(expr);
                if let Some(range) = uses.found {
                    self.error = self.check.escapes(range, self.assigned).err();
                }
            }
            expr => walk_expr(self, expr),
        }
    }
}

/// The visitor that finds the first use of `self` in a statement.
struct Uses<'a> {
    receiver: &'a str,
    found: Option<TextRange>,
}

impl<'a> Visitor<'a> for Uses<'_> {
    fn visit_stmt(&mut self, stmt: &'a ast::Stmt) {
        if self.found.is_none() {
            walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        match expr {
            ast::Expr::Name(name) if name.id.as_str() == self.receiver => {
                self.found.get_or_insert(name.range());
            }
            ast::Expr::Call(call) if is_super(call) => {
                self.found.get_or_insert(call.range());
            }
            expr => walk_expr(self, expr),
        }
    }
}

/// Returns `true` if `call` is `super()`, `false` otherwise.
fn is_super(call: &ast::ExprCall) -> bool {
    matches!(&*call.func, ast::Expr::Name(name) if name.id.as_str() == "super")
}

/// The call of `expr`, when `expr` is `super().__init__(...)`.
fn super_init(expr: &ast::Expr) -> Option<&ast::ExprCall> {
    let ast::Expr::Call(call) = expr else {
        return None;
    };
    let ast::Expr::Attribute(attr) = &*call.func else {
        return None;
    };
    match &*attr.value {
        ast::Expr::Call(sup) if attr.attr.as_str() == "__init__" && is_super(sup) => Some(call),
        _ => None,
    }
}
