//! Statements.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::call::is_builtins;
use super::flow::imported_name;
use super::{Lowering, params_of, unsupported};
use crate::Error;
use crate::cpython::Class;
use crate::ir::{Place, Rep, Stmt};

impl<'a> Lowering<'a, '_> {
    pub(super) fn body(&self, body: &[ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            if let ast::Stmt::FunctionDef(def) = stmt {
                lowered.extend(self.defaults(def)?);
            }
            lowered.extend(self.stmt(stmt)?);
        }
        Ok(lowered)
    }

    /// The assignment of the default of each parameter of `def`, which the
    /// `def` evaluates once, in the order of the parameters.
    fn defaults(&self, def: &ast::StmtFunctionDef) -> Result<Vec<Stmt>, Error> {
        let Some(id) = self.funcs.borrow().names.get(def.name.as_str()).copied() else {
            return Ok(Vec::new());
        };
        let sig = self.funcs.borrow().get(id).clone();
        params_of(def)
            .zip(&sig.defaults)
            .filter_map(|(param, var)| Some((param.default.as_deref()?, (*var)?)))
            .map(|(default, var)| {
                let value = self.expr(default)?;
                if value.rep() != self.vars.borrow().get(var).rep {
                    return Err(unsupported(default, "a default of this type"));
                }
                Ok(Stmt::Assign {
                    targets: vec![Place::Global(var)],
                    value,
                })
            })
            .collect()
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
                    .names
                    .get(def.name.as_str())
                    .expect("a declared function");
                Ok(Some(Stmt::Def(id)))
            }
            ast::Stmt::Return(r) => self.return_(r).map(Some),
            ast::Stmt::Raise(r) => self.raise(r).map(Some),
            ast::Stmt::Assert(a) => {
                let arg = match a.msg.as_deref() {
                    Some(msg) => {
                        let lowered = self.expr(msg)?;
                        if lowered.rep() != Rep::Str {
                            return Err(unsupported(msg, "an exception with this argument"));
                        }
                        Some(lowered)
                    }
                    None => None,
                };
                // The traceback marks the condition, and the program
                // evaluates the text only when the condition is false.
                Ok(Some(Stmt::If {
                    cond: self.truth_of(&a.test)?,
                    then: Vec::new(),
                    else_: vec![Stmt::Raise {
                        class: Class::AssertionError,
                        arg,
                        site: self.site(a.test.range()),
                    }],
                }))
            }
            ast::Stmt::Import(i) => {
                if let Some(alias) = i.names.iter().find(|alias| alias.name.as_str() != "math") {
                    return Err(unsupported(alias, "an import of this module"));
                }
                self.import(stmt, &i.names, false)
            }
            ast::Stmt::ImportFrom(i) => {
                if i.level != 0
                    || i.module
                        .as_ref()
                        .is_none_or(|module| module.as_str() != "math")
                {
                    return Err(unsupported(stmt, "an import from this module"));
                }
                if let Some(alias) = i.names.iter().find(|alias| alias.name.as_str() == "*") {
                    return Err(unsupported(alias, "an import of every name"));
                }
                self.import(stmt, &i.names, true)
            }
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    /// The import of the names of `aliases`, which the module declares.
    fn import(
        &self,
        stmt: &ast::Stmt,
        aliases: &[ast::Alias],
        from: bool,
    ) -> Result<Option<Stmt>, Error> {
        if self.scope.borrow().is_some() {
            return Err(unsupported(stmt, "an import in a function"));
        }
        let imports = self.imports.borrow();
        let ids = aliases
            .iter()
            .map(|alias| {
                *imports
                    .ids
                    .get(imported_name(alias, from))
                    .expect("a declared import")
            })
            .collect();
        Ok(Some(Stmt::Import(ids)))
    }

    /// `raise C`, `raise C()` or `raise C(text)` of a class of the builtins,
    /// with a `str` as the text.
    fn raise(&self, r: &ast::StmtRaise) -> Result<Stmt, Error> {
        let (Some(exc), None) = (r.exc.as_deref(), &r.cause) else {
            return Err(unsupported(r, "this raise"));
        };
        let (class, args) = match exc {
            ast::Expr::Call(call) if call.arguments.keywords.is_empty() => {
                (&*call.func, &*call.arguments.args)
            }
            ast::Expr::Call(_) => return Err(unsupported(exc, "a raise with these arguments")),
            class => (class, &[][..]),
        };
        let class = match expr_shape(self.model(), class) {
            Some(Shape::Class(id)) if is_builtins(self.model().db(), id.file) => {
                Class::of(&id.name)
            }
            _ => None,
        };
        let Some(class) = class else {
            return Err(unsupported(exc, "a raise of this exception"));
        };
        let arg = match args {
            [] => None,
            [arg] => {
                let lowered = self.expr(arg)?;
                if lowered.rep() != Rep::Str {
                    return Err(unsupported(arg, "an exception with this argument"));
                }
                Some(lowered)
            }
            _ => return Err(unsupported(exc, "a raise with these arguments")),
        };
        Ok(Stmt::Raise {
            class,
            arg,
            site: self.site(r.range()),
        })
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
