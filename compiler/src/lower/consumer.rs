//! The builtins that consume an iterable: `sum`, `min`, `max`, `any` and
//! `all`, which the module `consumers` of the prelude gives. Each one takes
//! a list or a generator expression.

use ruff_python_ast as ast;
use ty_python_semantic::types::lowering::expr_shape;

use super::generator::Consume;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, MinMaxOp, Rep};

impl Lowering<'_, '_> {
    /// `sum(xs)` of a list of `int`, of `bool` or of `float`.
    pub(super) fn sum(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of sum"));
        };
        let list = self.consumed_list(call, arg)?;
        let name = match list.rep().list().map(|list| self.list_item(list)) {
            Some(Rep::Int | Rep::Bool) => "sum_int",
            Some(Rep::Float) => "sum_float",
            Some(_) => return Err(unsupported(arg, "sum of a list of this type")),
            None => return Err(unsupported(arg, "sum of this iterable")),
        };
        self.module_call(call, "consumers", name, vec![list])
    }

    /// `min(xs)` or `max(xs)` of `arg`, a list of numbers, of `str`s or of
    /// objects.
    pub(super) fn min_max_of(
        &self,
        call: &ast::ExprCall,
        arg: &ast::Expr,
        op: MinMaxOp,
    ) -> Result<Expr, Error> {
        let list = self.consumed_list(call, arg)?;
        match list.rep().list().map(|list| self.list_item(list)) {
            // CPython compares each value as the generator gives it, and a
            // method of a class may write between two values.
            Some(Rep::Object(_)) if arg.is_generator_expr() => {
                return Err(unsupported(arg, "min or max of a generator of objects"));
            }
            Some(Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::Object(_)) => {}
            Some(_) => return Err(unsupported(arg, "min or max of a list of this type")),
            None => return Err(unsupported(arg, "min or max of this iterable")),
        }
        let name = match op {
            MinMaxOp::Min => "min_of",
            MinMaxOp::Max => "max_of",
        };
        self.module_call(call, "consumers", name, vec![list])
    }

    /// `any(xs)` or `all(xs)` of a list, which `name` gives, or of a
    /// generator expression, whose function stops at the first value that
    /// decides.
    pub(super) fn any_all(&self, call: &ast::ExprCall, name: &str) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of any or all"));
        };
        if let ast::Expr::Generator(generator) = arg {
            // CPython takes the truth in the frame of the code around the
            // generator, and the truth of an object may call a method.
            if matches!(
                self.rep_of(expr_shape(self.model(), &generator.elt)),
                Some(Rep::Object(_) | Rep::ObjectOrNone(_) | Rep::Uniform)
            ) {
                return Err(unsupported(arg, "any or all of a generator of objects"));
            }
            let consume = if name == "any_of" {
                Consume::Any
            } else {
                Consume::All
            };
            return self.generator(call, generator, consume);
        }
        let list = self.expr(arg)?;
        if list.rep().list().is_none() {
            return Err(unsupported(arg, "any or all of this iterable"));
        }
        self.module_call(call, "consumers", name, vec![list])
    }
}
