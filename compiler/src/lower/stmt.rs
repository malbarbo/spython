//! Statements.

use ruff_python_ast as ast;

use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Rep, Stmt};

impl<'a> Lowering<'a, '_> {
    pub(super) fn body(&self, body: &[ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            lowered.extend(self.stmt(stmt)?);
        }
        Ok(lowered)
    }

    fn stmt(&self, stmt: &ast::Stmt) -> Result<Option<Stmt>, Error> {
        match stmt {
            ast::Stmt::Expr(e) => Ok(Some(Stmt::Expr(self.expr(&e.value)?))),
            ast::Stmt::Assign(a) => {
                let value = self.expr(&a.value)?;
                let targets = a
                    .targets
                    .iter()
                    .map(|target| self.target(target, &value))
                    .collect::<Result<_, _>>()?;
                Ok(Some(Stmt::Assign { targets, value }))
            }
            // An annotation alone binds nothing.
            ast::Stmt::AnnAssign(ast::StmtAnnAssign { value: None, .. }) => Ok(None),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                let value = self.expr(value)?;
                let target = self.target(target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::AugAssign(a) => {
                let ast::Expr::Name(name) = &*a.target else {
                    return Err(unsupported(&*a.target, "this target"));
                };
                let left = self.load(name)?;
                let right = self.expr(&a.value)?;
                let value = self.binary(a, a.op, (left, name), (right, &*a.value))?;
                let target = self.target(&a.target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::If(i) => {
                let mut else_ = Vec::new();
                // Each `elif` is an `if` in the `else` of the clause before it.
                for clause in i.elif_else_clauses.iter().rev() {
                    let body = self.body(&clause.body)?;
                    else_ = match &clause.test {
                        Some(test) => vec![Stmt::If {
                            cond: self.truth_of(test)?,
                            then: body,
                            else_,
                        }],
                        None => body,
                    };
                }
                Ok(Some(Stmt::If {
                    cond: self.truth_of(&i.test)?,
                    then: self.body(&i.body)?,
                    else_,
                }))
            }
            ast::Stmt::While(w) => Ok(Some(Stmt::While {
                cond: self.truth_of(&w.test)?,
                body: self.body(&w.body)?,
                else_: self.body(&w.orelse)?,
            })),
            ast::Stmt::Break(_) => Ok(Some(Stmt::Break)),
            ast::Stmt::Continue(_) => Ok(Some(Stmt::Continue)),
            // `declare_vars` and `func` read the names.
            ast::Stmt::Pass(_) | ast::Stmt::Global(_) => Ok(None),
            ast::Stmt::FunctionDef(def) => {
                if self.scope.borrow().is_some() {
                    return Err(unsupported(def, "a def in a function"));
                }
                let id = *self
                    .funcs
                    .borrow()
                    .ids
                    .get(def.name.as_str())
                    .expect("a declared function");
                Ok(Some(Stmt::Def(id)))
            }
            ast::Stmt::Return(r) => self.return_(r).map(Some),
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    fn return_(&self, r: &ast::StmtReturn) -> Result<Stmt, Error> {
        let Some(ret) = self.scope.borrow().as_ref().map(|scope| scope.ret) else {
            return Err(unsupported(r, "a return outside a function"));
        };
        match r.value.as_deref() {
            None | Some(ast::Expr::NoneLiteral(_)) if ret == Rep::None => Ok(Stmt::Return(None)),
            None | Some(ast::Expr::NoneLiteral(_)) => Err(unsupported(r, "a return of None")),
            Some(value) => {
                let lowered = self.expr(value)?;
                if lowered.rep() != ret {
                    return Err(unsupported(value, "a return of this type"));
                }
                Ok(Stmt::Return(Some(lowered)))
            }
        }
    }
}
