//! The builtins that consume an iterable: `sum`, `min`, `max`, `any`,
//! `all` and `sorted`, which the modules `consumers` and `sort` of the
//! prelude give, and the method `sort` of a list. Each builtin takes a list
//! or a generator expression.

use ruff_python_ast as ast;
use ty_python_semantic::types::lowering::expr_shape;

use super::generator::Consume;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, MinMaxOp, Place, Rep, Stmt};

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

impl Lowering<'_, '_> {
    /// `xs.sort(key=key, reverse=reverse)` of `list`, the value of `xs`.
    pub(super) fn list_sort(&self, call: &ast::ExprCall, list: Expr) -> Result<Expr, Error> {
        if !call.arguments.args.is_empty() {
            return Err(unsupported(call, "a call of sort with these arguments"));
        }
        let (key, reverse) = self.sort_options(call)?;
        self.sort_call(call, list, key, reverse, false)
    }

    /// `sorted(xs, key=key, reverse=reverse)` of a list, which sorts a copy,
    /// or of a generator expression, which sorts the list of its values.
    /// The first iterable of a generator runs before `key` and `reverse`,
    /// and the generator after them, as in CPython.
    pub(super) fn sorted(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let [arg] = &*call.arguments.args else {
            return Err(unsupported(call, "a call of sorted with these arguments"));
        };
        let ast::Expr::Generator(generator) = arg else {
            let list = self.expr(arg)?;
            if list.rep().list().is_none() {
                return Err(unsupported(arg, "sorted of this iterable"));
            }
            let (key, reverse) = self.sort_options(call)?;
            return self.sort_call(call, list, key, reverse, true);
        };
        let (mut stmts, values) = self.generator_call(call, generator, Consume::List)?;
        let (key, reverse) = self.sort_options(call)?;
        let mut hold = |value: Expr| {
            let rep = value.rep();
            let var = self.hidden_var(rep);
            stmts.push(Stmt::Assign {
                targets: vec![var],
                value,
            });
            (var, rep)
        };
        let read = |(place, rep): (Place, Rep)| Expr::Load {
            place,
            rep,
            check: None,
        };
        let key = key.map(|key| read(hold(key)));
        let reverse = read(hold(reverse));
        let list = hold(values);
        stmts.push(Stmt::Expr(self.sort_call(
            call,
            read(list),
            key,
            reverse,
            false,
        )?));
        Ok(Expr::Block {
            stmts,
            value: Box::new(read(list)),
        })
    }

    /// The `key` and the `reverse` of `call`, a call of `sort` or of
    /// `sorted`, in the order of the call. A `key` of `None` is no key.
    fn sort_options(&self, call: &ast::ExprCall) -> Result<(Option<Expr>, Expr), Error> {
        let mut key = None;
        let mut reverse = Expr::Bool(false);
        for keyword in &call.arguments.keywords {
            match keyword.arg.as_ref().map(ast::Identifier::as_str) {
                Some("key") if keyword.value.is_none_literal_expr() => key = None,
                Some("key") => {
                    let value = self.expr(&keyword.value)?;
                    let Rep::Func(ty) = value.rep() else {
                        return Err(unsupported(&keyword.value, "a key of this type"));
                    };
                    let ret = self.func_type_of(ty).ret;
                    if !matches!(
                        ret,
                        Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::Object(_)
                    ) {
                        return Err(unsupported(&keyword.value, "a key of this type"));
                    }
                    key = Some(value);
                }
                Some("reverse") => {
                    reverse = self.expr(&keyword.value)?;
                    if reverse.rep() != Rep::Bool {
                        return Err(unsupported(&keyword.value, "a reverse of this type"));
                    }
                }
                _ => return Err(unsupported(keyword, "this keyword argument")),
            }
        }
        Ok((key, reverse))
    }

    /// The call at `call` of the function of the module `sort` of the
    /// prelude that sorts `list`, or a copy of it when `copy` holds.
    fn sort_call(
        &self,
        call: &ast::ExprCall,
        list: Expr,
        key: Option<Expr>,
        reverse: Expr,
        copy: bool,
    ) -> Result<Expr, Error> {
        let (name, args) = match key {
            Some(key) => (
                if copy { "sorted_key" } else { "sort_key" },
                vec![list, key, reverse],
            ),
            None => (if copy { "sorted_of" } else { "sort" }, vec![list, reverse]),
        };
        self.module_call(call, "sort", name, args)
    }
}
