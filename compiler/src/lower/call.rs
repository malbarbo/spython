//! Calls of the functions of the module and of the builtins.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::expr::float_value;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, MinMaxOp, Rep};

impl<'a> Lowering<'a, '_> {
    /// A call of a function of the module, with positional arguments.
    pub(super) fn call(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ast::Expr::Name(name) = &*call.func else {
            return Err(unsupported(
                call,
                "a call of an expression other than a name",
            ));
        };
        let callee = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(callee))
        {
            return Err(unsupported(call, "a call of a local variable"));
        }
        let Some(func) = self.funcs.borrow().ids.get(callee).copied() else {
            return Err(unsupported(
                call,
                "a call of a function that the module does not define",
            ));
        };
        // The `def` is the one binding of the name in the module, so the call
        // finds the function, or raises `NameError` before the `def` runs.
        let sig = self.funcs.borrow().get(func).clone();
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "a call with keyword arguments"));
        }
        if call.arguments.args.len() != sig.params.len() {
            return Err(unsupported(call, "a call with this number of arguments"));
        }
        let check = self.global_check(name)?;
        let args = call
            .arguments
            .args
            .iter()
            .zip(&sig.params)
            .map(|(arg, rep)| {
                let lowered = self.expr(arg)?;
                if lowered.rep() != *rep {
                    return Err(unsupported(arg, "an argument of this type"));
                }
                Ok(lowered)
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Call {
            func,
            args,
            rep: sig.ret,
            site: self.site(call.range()),
            check,
        })
    }

    pub(super) fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::None => Err(unsupported(arg, "print of None")),
                    _ => Ok(lowered),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print {
            args,
            site: self.site(call.range()),
        })
    }

    /// `len` of one `str`.
    pub(super) fn len(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of len"));
        };
        let operand = self.expr(arg)?;
        if operand.rep() != Rep::Str {
            return Err(unsupported(arg, "len of this type"));
        }
        Ok(Expr::Len {
            operand: Box::new(operand),
            site: self.site(call.range()),
        })
    }

    /// `abs` of one number, where a `bool` counts as an `int`.
    pub(super) fn abs(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of abs")?;
        let operand = self.expr(arg)?;
        match operand.rep() {
            Rep::Float => Ok(Expr::FloatAbs(Box::new(operand))),
            Rep::Int | Rep::Bool => Ok(Expr::IntAbs {
                operand: Box::new(self.int_value(operand, arg)?),
                site: self.site(call.range()),
            }),
            Rep::Str | Rep::None => Err(unsupported(arg, "abs of this type")),
        }
    }

    /// `min` or `max` of two or more values of one type. An `int` and a
    /// `bool` are two types here, because the result keeps its type.
    pub(super) fn min_max(&self, call: &ast::ExprCall, op: MinMaxOp) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "min or max with keyword arguments"));
        }
        let [first, rest @ ..] = &*call.arguments.args else {
            return Err(unsupported(call, "min or max of an iterable"));
        };
        if rest.is_empty() {
            return Err(unsupported(call, "min or max of an iterable"));
        }
        let first = self.expr(first)?;
        let rep = first.rep();
        if rep == Rep::None {
            return Err(unsupported(call, "min or max of None"));
        }
        let mut args = vec![first];
        for arg in rest {
            let lowered = self.expr(arg)?;
            if lowered.rep() != rep {
                return Err(unsupported(call, "min or max of values of two types"));
            }
            args.push(lowered);
        }
        Ok(Expr::MinMax { op, args })
    }

    /// `pow(a, b)`, which is `a ** b`.
    pub(super) fn pow(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([base, exp], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of pow"));
        };
        let left = self.expr(base)?;
        let right = self.expr(exp)?;
        self.binary(call, ast::Operator::Pow, (left, base), (right, exp))
    }

    /// `round(x)` or `round(x, None)` of a number, which gives an `int`.
    /// Only the literal `None` stands for the missing number of digits.
    pub(super) fn round(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = match (&*call.arguments.args, &*call.arguments.keywords) {
            ([arg] | [arg, ast::Expr::NoneLiteral(_)], []) => arg,
            _ => return Err(unsupported(call, "this call of round")),
        };
        let operand = self.expr(arg)?;
        match operand.rep() {
            Rep::Int | Rep::Bool => self.int_value(operand, arg),
            Rep::Float => Ok(Expr::FloatAsInt {
                operand: Box::new(Expr::FloatRound(Box::new(operand))),
                site: self.site(call.range()),
            }),
            Rep::Str | Rep::None => Err(unsupported(arg, "round of this type")),
        }
    }

    /// `bool()`, which is `False`, or `bool(x)`, which is the truth of `x`.
    pub(super) fn bool(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Bool(false)),
            ([arg], []) => self.truth_of(arg),
            _ => Err(unsupported(call, "this call of bool")),
        }
    }

    /// `float()`, which is `0.0`, or `float(x)` of a number.
    pub(super) fn float(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Float(0.0)),
            ([arg], []) => {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::Int | Rep::Bool | Rep::Float => float_value(lowered, arg),
                    Rep::Str | Rep::None => Err(unsupported(arg, "float of this type")),
                }
            }
            _ => Err(unsupported(call, "this call of float")),
        }
    }

    /// `int()`, which is `0`, or `int(x)` of a number.
    pub(super) fn int(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Int(0)),
            ([arg], []) => {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::Int | Rep::Bool => self.int_value(lowered, arg),
                    Rep::Float => Ok(Expr::FloatAsInt {
                        operand: Box::new(lowered),
                        site: self.site(call.range()),
                    }),
                    Rep::Str | Rep::None => Err(unsupported(arg, "int of this type")),
                }
            }
            _ => Err(unsupported(call, "this call of int")),
        }
    }

    /// Returns `true` if `func` is the class `name` of the builtins, `false`
    /// otherwise.
    pub(super) fn is_class(&self, func: &ast::Expr, name: &str) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Class(class)) if class.name == name && is_builtins(self.model.db(), class.file)
        )
    }

    /// Returns `true` if `func` is the function `name` of the builtins,
    /// `false` otherwise.
    pub(super) fn is_builtin(&self, func: &ast::Expr, name: &str) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Function(def)) if def.name == name && is_builtins(self.model.db(), def.file)
        )
    }
}

/// The one positional argument of `call`, which has no keyword argument.
fn one_arg<'c>(call: &'c ast::ExprCall, what: &str) -> Result<&'c ast::Expr, Error> {
    match (&*call.arguments.args, &*call.arguments.keywords) {
        ([arg], []) => Ok(arg),
        _ => Err(unsupported(call, what)),
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}
