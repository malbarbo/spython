//! Statements.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::call::is_builtins;
use super::flow::imported_name;
use super::{Lowering, coerce, params_of, unsupported};
use crate::Error;
use crate::cpython::Class;
use crate::ir::{Expr, Place, Rep, Stmt};

/// The modules that a program may import. No import runs code in the
/// compiled program.
const MODULES: [&str; 3] = ["math", "enum", "dataclasses"];

impl<'a> Lowering<'a, '_> {
    pub(super) fn body(&self, body: &[ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            if let ast::Stmt::FunctionDef(def) = stmt
                && self.scope.borrow().is_none()
            {
                lowered.extend(self.defaults(def)?);
                let annotations = params_of(def)
                    .filter_map(|param| param.annotation())
                    .chain(def.returns.as_deref());
                self.check_annotations(annotations)?;
            }
            // A function does not evaluate the annotation of an assignment.
            if let ast::Stmt::AnnAssign(a) = stmt
                && self.scope.borrow().is_none()
            {
                self.check_annotations(std::iter::once(&*a.annotation))?;
            }
            if let ast::Stmt::ClassDef(def) = stmt {
                lowered.extend(self.class_body(def)?);
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
                let rep = self.vars.borrow().get(var).rep;
                let value = coerce(self.expr(default)?, rep)
                    .ok_or_else(|| unsupported(default, "a default of this type"))?;
                Ok(Stmt::Assign {
                    targets: vec![Place::Global(var)],
                    value,
                })
            })
            .collect()
    }

    fn stmt(&self, stmt: &ast::Stmt) -> Result<Option<Stmt>, Error> {
        match stmt {
            ast::Stmt::Expr(e) => {
                let value = self.expr(&e.value)?;
                // An example of a docstring writes the `repr` of the value
                // of each expression statement that is not `None`, as the
                // interactive prompt does.
                if self.in_doctest() && value.rep() != Rep::None {
                    // The prompt writes nothing for a value that is `None`
                    // when the example runs.
                    if matches!(
                        value.rep(),
                        Rep::StrOrNone | Rep::Uniform | Rep::ListOrNone(_)
                    ) {
                        return Err(unsupported(&*e.value, "an example whose value may be None"));
                    }
                    let text = self.repr_of(value, e.range())?;
                    return Ok(Some(Stmt::Expr(Expr::Print {
                        args: vec![text],
                        site: self.site(e.range()),
                    })));
                }
                Ok(Some(Stmt::Expr(value)))
            }
            ast::Stmt::Assign(a)
                if self.in_prelude() && a.targets.iter().any(|t| t.is_subscript_expr()) =>
            {
                let [ast::Expr::Subscript(target)] = &*a.targets else {
                    return Err(unsupported(a, "this target"));
                };
                // The one subscript that the prelude assigns is a code
                // point of a builder.
                Ok(Some(Stmt::StrSet {
                    builder: self.expr(&target.value)?,
                    index: self.expr(&target.slice)?,
                    value: self.expr(&a.value)?,
                }))
            }
            ast::Stmt::Assign(a) if self.module_copy(&a.targets, &a.value).is_some() => {
                let (target, from) = self
                    .module_copy(&a.targets, &a.value)
                    .expect("a copy of a name of the module");
                let (from, rep) = self
                    .vars
                    .borrow()
                    .find(from)
                    .ok_or_else(|| unsupported(&*a.value, "a variable of this type"))?;
                // The copy asks whether `from` has a value.
                self.vars.borrow_mut().get_mut(from).flagged = true;
                let (to, _) = self.place(target, rep)?;
                Ok(Some(Stmt::Copy { from, to }))
            }
            ast::Stmt::Assign(a) => {
                let value = self.expr(&a.value)?;
                if let [ast::Expr::Attribute(attr)] = &*a.targets {
                    return self.assign_field(attr, value).map(Some);
                }
                let targets = a
                    .targets
                    .iter()
                    .map(|target| Ok(self.place(target, value.rep())?.0))
                    .collect::<Result<_, Error>>()?;
                Ok(Some(Stmt::Assign { targets, value }))
            }
            // An annotation alone binds nothing.
            ast::Stmt::AnnAssign(ast::StmtAnnAssign { value: None, .. }) => Ok(None),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                let (target, value) = self.target(target, self.expr(value)?)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::AugAssign(ast::StmtAugAssign {
                target, op, value, ..
            }) if let ast::Expr::Attribute(attr) = &**target => {
                let (object, class, field, rep) = self.updated_field(attr)?;
                let left = self.object_attribute(attr, Expr::Target(class), class)?;
                let left = self.narrowed_field(target, attr, left)?;
                let right = self.expr(value)?;
                let value = self.binary(stmt, *op, (left, &**target), (right, &**value))?;
                let value = coerce(value, rep)
                    .ok_or_else(|| unsupported(attr, "a field with values of two types"))?;
                Ok(Some(Stmt::UpdateField {
                    object,
                    class,
                    field,
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
                let (target, value) = self.target(&a.target, value)?;
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
            ast::Stmt::ClassDef(def) => Ok(Some(Stmt::Class(self.class_of(def)?))),
            ast::Stmt::Return(r) => self.return_(r).map(Some),
            ast::Stmt::Raise(r) => self.raise(r).map(Some),
            ast::Stmt::Assert(a) => {
                let arg = a
                    .msg
                    .as_deref()
                    .map(|msg| self.exception_arg(msg))
                    .transpose()?;
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
                if let Some(alias) = i
                    .names
                    .iter()
                    .find(|alias| !MODULES.contains(&alias.name.as_str()))
                {
                    return Err(unsupported(alias, "an import of this module"));
                }
                self.import(stmt, &i.names, false)
            }
            ast::Stmt::ImportFrom(i) => {
                let module = i.module.as_ref().map(|module| module.as_str());
                let Some(module) = module.filter(|module| i.level == 0 && MODULES.contains(module))
                else {
                    return Err(unsupported(stmt, "an import from this module"));
                };
                if let Some(alias) = i.names.iter().find(|alias| alias.name.as_str() == "*") {
                    return Err(unsupported(alias, "an import of every name"));
                }
                // The compiler reads `Enum`, `auto` and `dataclass` in the
                // form of a class alone, and every name of `math` at its use.
                let known = |name: &str| match module {
                    "enum" => ["Enum", "auto"].contains(&name),
                    "dataclasses" => name == "dataclass",
                    _ => true,
                };
                if let Some(alias) = i.names.iter().find(|alias| !known(alias.name.as_str())) {
                    return Err(unsupported(alias, "an import of this name"));
                }
                self.import(stmt, &i.names, true)
            }
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    /// The target and the name of the global of `a`, when `a` is a copy
    /// of a name of the module that the examples of the docstrings assign.
    /// The runner gives the examples a copy of the globals of the module,
    /// and a name without a value stays without one in the copy.
    pub(super) fn module_copy<'s>(
        &self,
        targets: &'s [ast::Expr],
        value: &'s ast::Expr,
    ) -> Option<(&'s ast::Expr, &'s str)> {
        let ([target @ ast::Expr::Name(to)], ast::Expr::Name(from)) = (targets, value) else {
            return None;
        };
        let copies = self.tests.as_ref()?.copies;
        let copy = |name: &ast::ExprName| copies.iter().any(|copy| *copy == name.id.as_str());
        (copy(to) || copy(from)).then_some((target, from.id.as_str()))
    }

    /// The text of an exception that `arg` gives, which has to be a `str`.
    fn exception_arg(&self, arg: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(arg)?;
        if lowered.rep() != Rep::Str {
            return Err(unsupported(arg, "an exception with this argument"));
        }
        Ok(lowered)
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
            [arg] => Some(self.exception_arg(arg)?),
            _ => return Err(unsupported(exc, "a raise with these arguments")),
        };
        Ok(Stmt::Raise {
            class,
            arg,
            site: self.site(r.range()),
        })
    }

    fn return_(&self, r: &ast::StmtReturn) -> Result<Stmt, Error> {
        let ret = match self.scope.borrow().as_ref() {
            Some(scope) if scope.doctest.is_none() => scope.ret,
            _ => return Err(unsupported(r, "a return outside a function")),
        };
        match r.value.as_deref() {
            None | Some(ast::Expr::NoneLiteral(_)) if ret == Rep::None => Ok(Stmt::Return(None)),
            None => coerce(Expr::None, ret)
                .map(|value| Stmt::Return(Some(value)))
                .ok_or_else(|| unsupported(r, "a return of None")),
            Some(value) => coerce(self.expr(value)?, ret)
                .map(|value| Stmt::Return(Some(value)))
                .ok_or_else(|| unsupported(value, "a return of this type")),
        }
    }
}
