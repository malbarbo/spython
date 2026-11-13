//! Functions as values: the name of a function of the module and a
//! `lambda`, with the variables that it captures.

use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::lowering::expr_shape;

use super::capture::{Captures, free_names_of_lambda};
use super::class::param;
use super::{Lowering, Scope, ScopeCapture, Vars, check_self, unsupported};
use crate::Error;
use crate::ir::{
    Binding, Capture, Captured, Check, CmpOp, Expr, Func, FuncId, Place, Rep, Stmt, Undefined,
};

impl Lowering<'_, '_> {
    /// The value of `lambda`, the node `expr`, whose code is a function of
    /// its own, with the frame `<lambda>` in a traceback. The checker gives
    /// the type of each parameter from where the `lambda` goes.
    pub(super) fn lambda(&self, expr: &ast::Expr, lambda: &ast::ExprLambda) -> Result<Expr, Error> {
        // The frame of a `lambda` in an example of a docstring is in the
        // file of the example.
        if self.in_doctest() {
            return Err(unsupported(expr, "a lambda in a doctest"));
        }
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.class.is_some())
        {
            return Err(unsupported(expr, "a lambda in the body of a class"));
        }
        let Some(Rep::Func(ty)) = self.rep_of(expr_shape(self.model(), expr)) else {
            return Err(unsupported(expr, "a lambda of this type"));
        };
        let names = lambda_params(lambda)?;
        let ty_of = self.func_type_of(ty);
        let (captured, captures) =
            self.closure_captures(expr, lambda.range(), free_names_of_lambda(lambda))?;
        let mut vars = Vars::default();
        for (name, rep) in names.iter().zip(&ty_of.params) {
            check_self(name.as_str(), *rep, *name)?;
            vars.add(name.as_str(), *rep);
        }
        let scope = Scope {
            params: names.len(),
            doctest: None,
            name: "<lambda>".to_owned(),
            ret: ty_of.ret,
            locals: names.iter().map(|name| name.to_string()).collect(),
            order: names.iter().map(|name| name.to_string()).collect(),
            vars,
            namespace: Vec::new(),
            class: None,
            inner: Captures::default(),
            captures: captures.clone(),
        };
        let (scope, body) = self.in_scope(scope, || {
            let value = self.expr(&lambda.body)?;
            Ok(match ty_of.ret {
                Rep::None => vec![Stmt::Expr(value), Stmt::Return(None)],
                ret => {
                    let value = self
                        .coerce(value, ret)
                        .ok_or_else(|| unsupported(&*lambda.body, "a lambda of this type"))?;
                    vec![Stmt::Return(Some(value))]
                }
            })
        });
        let body = body?;
        let code = self.reserve(ty_of.params.clone(), ty_of.ret);
        self.lowered.borrow_mut().insert(
            code.index(),
            Func {
                value: Some(ty),
                captures: captures
                    .iter()
                    .map(|capture| Capture {
                        rep: capture.rep,
                        cell: capture.cell,
                    })
                    .collect(),
                alias: None,
                ordered: false,
                frame: true,
                params: u32::try_from(names.len()).expect("fewer than 2^32 parameters"),
                vars: scope.vars.list,
                ret: ty_of.ret,
                body,
            },
        );
        Ok(Expr::Closure {
            func: code,
            ty,
            captures: Some(captured),
        })
    }

    /// What a new value of the function at `range`, inside the function
    /// that the lowering is in, holds for each variable of that function
    /// among the `names` that it reads, and the captures of its code. A
    /// function of the module captures nothing.
    fn closure_captures(
        &self,
        node: &impl Ranged,
        range: TextRange,
        names: Vec<&str>,
    ) -> Result<(Vec<Captured>, Vec<ScopeCapture>), Error> {
        let scope = self.scope.borrow();
        let Some(scope) = scope.as_ref() else {
            return Ok((Vec::new(), Vec::new()));
        };
        let mut captured = Vec::new();
        let mut captures = Vec::new();
        for name in names {
            let (value, capture) = if scope.is_local(name) {
                let (var, rep) = scope
                    .vars
                    .find(name)
                    .ok_or_else(|| unsupported(node, "a capture of a variable of this type"))?;
                if rep == Rep::None {
                    return Err(unsupported(node, "a capture of a variable of None"));
                }
                let cell = scope.inner.cells.contains(name);
                let unbound = scope
                    .inner
                    .funcs
                    .get(&range)
                    .and_then(|names| names.iter().find(|(found, _)| found == name))
                    .is_some_and(|(_, unbound)| *unbound);
                let value = if cell {
                    Captured::Cell(var)
                } else {
                    Captured::Value(Expr::Load {
                        place: Place::Local(var),
                        rep,
                        check: None,
                    })
                };
                let capture = ScopeCapture {
                    name: name.to_owned(),
                    rep,
                    cell,
                    unbound,
                };
                (value, capture)
            } else if let Some((index, outer)) = scope
                .captures
                .iter()
                .enumerate()
                .find(|(_, capture)| capture.name == name)
            {
                let index = u32::try_from(index).expect("fewer than 2^32 captures");
                let value = if outer.cell {
                    Captured::Outer(index)
                } else {
                    Captured::Value(Expr::Capture {
                        index,
                        rep: outer.rep,
                        check: None,
                    })
                };
                (value, outer.clone())
            } else {
                continue;
            };
            captured.push(value);
            captures.push(capture);
        }
        Ok((captured, captures))
    }

    /// The read `name` of a variable that the function that the lowering
    /// is in captures, and `None` for a name that it does not capture.
    pub(super) fn captured(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        let (index, capture) = {
            let scope = self.scope.borrow();
            let (index, capture) = scope
                .as_ref()?
                .captures
                .iter()
                .enumerate()
                .find(|(_, capture)| capture.name == id)?;
            (index, capture.clone())
        };
        let check = capture.unbound.then(|| {
            let mut error = self.name_error(id);
            let base = format!("name '{id}' is not defined");
            let free = format!(
                "cannot access free variable '{id}' where it is not associated with a value \
                 in enclosing scope"
            );
            error.text = error.text.replacen(&base, &free, 1);
            error.suggested.0 = error.suggested.0.replacen(&base, &free, 1);
            Check {
                site: self.site(name.range()),
                error: Undefined::Global(Box::new(error)),
            }
        });
        Some(Expr::Capture {
            index: u32::try_from(index).expect("fewer than 2^32 captures"),
            rep: capture.rep,
            check,
        })
    }

    /// Returns `true` if the function that the lowering is in captures
    /// `name`, `false` otherwise.
    pub(super) fn captures(&self, name: &str) -> bool {
        self.scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.captures.iter().any(|capture| capture.name == name))
    }

    /// What `lower` gives in `scope`, and the scope after it, with the
    /// scope of the code around it back in its place.
    fn in_scope<T>(
        &self,
        scope: Scope,
        lower: impl FnOnce() -> Result<T, Error>,
    ) -> (Scope, Result<T, Error>) {
        let outer = self.scope.replace(Some(scope));
        let result = lower();
        let inner = self.scope.replace(outer).expect("the scope of the code");
        (inner, result)
    }

    /// The value of the function of the module that `name` reads. A name
    /// with more than one `def` has the function of the last `def` that
    /// ran.
    pub(super) fn func_value(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let id = name.id.as_str();
        let redefined = self.funcs.borrow().redefined.get(id).cloned();
        let Some((current, defs)) = redefined else {
            let func = *self
                .funcs
                .borrow()
                .names
                .get(id)
                .expect("a function of the module");
            let value = self.value_of(func);
            return self.after(Binding::Func(func), name, value);
        };
        let values: Vec<Expr> = defs.iter().map(|func| self.value_of(*func)).collect();
        let rep = values.first().expect("a name with defs").rep();
        if values.iter().any(|value| value.rep() != rep) {
            return Err(unsupported(name, "a name whose functions have two types"));
        }
        // The read of the global raises `NameError` before any `def` of the
        // name runs.
        let load_current = || Expr::Load {
            place: Place::Global(current),
            rep: Rep::Int,
            check: None,
        };
        let mut values = values.into_iter().enumerate().rev();
        let (_, last) = values.next().expect("a name with defs");
        let value = values.fold(last, |else_, (place, value)| Expr::IfElse {
            cond: Box::new(Expr::Compare(
                Box::new(load_current()),
                vec![(
                    CmpOp::Eq,
                    Expr::Int(i32::try_from(place).expect("few defs")),
                )],
            )),
            then: Box::new(value),
            else_: Box::new(else_),
        });
        let check = self.global_check(name)?;
        Ok(match check {
            Some(check) => Expr::After {
                binding: Binding::Global(current),
                check,
                value: Box::new(value),
            },
            None => value,
        })
    }

    /// The value of `func`, a function of the module, whose code calls the
    /// function with the arguments of the call of the value.
    fn value_of(&self, func: FuncId) -> Expr {
        let sig = self.funcs.borrow().get(func).clone();
        let ty = self.func_type(sig.params.clone(), sig.ret);
        let code = *self.values.borrow_mut().entry(func).or_insert_with(|| {
            let args = sig
                .params
                .iter()
                .enumerate()
                .map(|(i, rep)| param(i, *rep))
                .collect();
            // The function of the module checks the limit of the recursion
            // and has the frame of its code.
            let call = Expr::Call {
                func,
                args,
                order: None,
                rep: sig.ret,
                site: None,
                check: None,
            };
            let body = match sig.ret {
                Rep::None => vec![Stmt::Expr(call), Stmt::Return(None)],
                _ => vec![Stmt::Return(Some(call))],
            };
            let code = self.reserve(sig.params.clone(), sig.ret);
            self.define_value(code, ty, body);
            code
        });
        Expr::Closure {
            func: code,
            ty,
            captures: None,
        }
    }
}

/// The name of each parameter of `lambda`, which takes positional
/// arguments alone, without defaults.
fn lambda_params(lambda: &ast::ExprLambda) -> Result<Vec<&ast::Identifier>, Error> {
    let Some(parameters) = lambda.parameters.as_deref() else {
        return Ok(Vec::new());
    };
    if !parameters.posonlyargs.is_empty()
        || parameters.vararg.is_some()
        || !parameters.kwonlyargs.is_empty()
        || parameters.kwarg.is_some()
        || parameters.args.iter().any(|param| param.default.is_some())
    {
        return Err(unsupported(parameters, "these parameters"));
    }
    Ok(parameters
        .args
        .iter()
        .map(|param| &param.parameter.name)
        .collect())
}
