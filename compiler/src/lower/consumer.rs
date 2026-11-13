//! The builtins that consume an iterable: `sum`, `min`, `max`, `any`,
//! `all` and `sorted`, which the modules `consumers` and `sort` of the
//! prelude give, and the method `sort` of a list. Each builtin takes a list
//! or a generator expression.

use ruff_python_ast as ast;
use ty_python_semantic::types::lowering::expr_shape;

use super::generator::Consume;
use super::{Lowering, TypeArg, read, unsupported};
use crate::Error;
use crate::ir::{Expr, MinMaxOp, Rep, Stmt};

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
        // CPython compares each value as the generator gives it, and a
        // method of a class may write between two values.
        if arg.is_generator_expr()
            && let Some(Rep::Object(_)) = list.rep().list().map(|list| self.list_item(list))
        {
            return Err(unsupported(arg, "min or max of a generator of objects"));
        }
        let list = self.ordered_list(arg, list)?;
        let name = match op {
            MinMaxOp::Min => "min_of",
            MinMaxOp::Max => "max_of",
        };
        self.module_call(call, "consumers", name, vec![list])
    }

    /// `min` or `max` with a `key`, of a list or of two or more values of
    /// one type, which go in a list. CPython calls `key` for each value as
    /// it comes, and compares the result with the result of the extreme so
    /// far, with `<` for `min` and `>` for `max`. A `key` of `None` is no
    /// key.
    pub(super) fn min_max_key(&self, call: &ast::ExprCall, op: MinMaxOp) -> Result<Expr, Error> {
        let list = match &*call.arguments.args {
            [] => return Err(unsupported(call, "min or max of an iterable")),
            // CPython calls `key` for each value as the generator gives it.
            [arg] if arg.is_generator_expr() => {
                return Err(unsupported(arg, "min or max of a generator with a key"));
            }
            [arg] => self.ordered_list(arg, self.expr(arg)?)?,
            args => {
                let items = args
                    .iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                let rep = items.first().map(Expr::rep).expect("two or more values");
                if !is_ordered(rep) {
                    return Err(unsupported(call, "min or max of values of this type"));
                }
                if items.iter().any(|item| item.rep() != rep) {
                    return Err(unsupported(call, "min or max of values of two types"));
                }
                let classes = Vec::new();
                let list = self.list_of(TypeArg { rep, classes });
                Expr::List { list, items }
            }
        };
        let key = match &*call.arguments.keywords {
            [keyword]
                if keyword
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.as_str() == "key") =>
            {
                self.optional_key(&keyword.value)?
            }
            _ => return Err(unsupported(call, "min or max with these keyword arguments")),
        };
        let (name, args) = match (op, key) {
            (MinMaxOp::Min, Some(key)) => ("min_key", vec![list, key]),
            (MinMaxOp::Max, Some(key)) => ("max_key", vec![list, key]),
            (MinMaxOp::Min, None) => ("min_of", vec![list]),
            (MinMaxOp::Max, None) => ("max_of", vec![list]),
        };
        self.module_call(call, "consumers", name, args)
    }

    /// `any(xs)` or `all(xs)`, which `consume` gives, of a list or of a
    /// generator expression, whose function stops at the first value that
    /// decides.
    pub(super) fn any_all(&self, call: &ast::ExprCall, consume: Consume) -> Result<Expr, Error> {
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
            return self.generator(call, generator, consume);
        }
        let list = self.expr(arg)?;
        if list.rep().list().is_none() {
            return Err(unsupported(arg, "any or all of this iterable"));
        }
        let name = match consume {
            Consume::Any => "any_of",
            Consume::All => "all_of",
            Consume::List => unreachable!("any or all"),
        };
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
        let key = key.map(|key| read(self.hold(&mut stmts, key)));
        let reverse = read(self.hold(&mut stmts, reverse));
        let list = self.hold(&mut stmts, values);
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
                Some("key") => key = self.optional_key(&keyword.value)?,
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

    /// `list`, the value of `arg`, the list of `min` or `max`, which has to
    /// hold items that the prelude compares.
    fn ordered_list(&self, arg: &ast::Expr, list: Expr) -> Result<Expr, Error> {
        match list.rep().list().map(|list| self.list_item(list)) {
            Some(item) if is_ordered(item) => Ok(list),
            Some(_) => Err(unsupported(arg, "min or max of a list of this type")),
            None => Err(unsupported(arg, "min or max of this iterable")),
        }
    }

    /// The `key` of `sort`, `sorted`, `min` or `max` at `node`, and `None`
    /// for a `key` of `None`, which is no key.
    fn optional_key(&self, node: &ast::Expr) -> Result<Option<Expr>, Error> {
        if node.is_none_literal_expr() {
            return Ok(None);
        }
        self.key(node).map(Some)
    }

    /// The function of `node`, the `key` of `sort`, `sorted`, `min` or
    /// `max`, whose results the prelude compares.
    fn key(&self, node: &ast::Expr) -> Result<Expr, Error> {
        let value = self.expr(node)?;
        let Rep::Func(ty) = value.rep() else {
            return Err(unsupported(node, "a key of this type"));
        };
        if !is_ordered(self.func_type_of(ty).ret) {
            return Err(unsupported(node, "a key of this type"));
        }
        Ok(value)
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
        // With a key, the prelude compares the results of the key, which
        // `key` checks.
        let item = list.rep().list().map(|list| self.list_item(list));
        if key.is_none() && !item.is_some_and(is_ordered) {
            return Err(unsupported(call, "sorted of a list of this type"));
        }
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

/// Returns `true` if the prelude compares values of `rep` with `<` and
/// `>`, `false` otherwise.
fn is_ordered(rep: Rep) -> bool {
    matches!(
        rep,
        Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::Object(_)
    )
}
