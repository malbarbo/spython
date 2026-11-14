//! The comprehensions, which run as a loop in line, as CPython 3.12 makes
//! them (PEP 709). The names of a comprehension are variables of its own,
//! and the traceback has no frame for it.

use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::lowering::expr_shape;

use super::class::generated_call;
use super::{Comprehension, Lowering, unsupported};
use crate::Error;
use crate::ir::{ClassId, Expr, Place, Rep, Stmt};

impl<'a> Lowering<'a, '_> {
    /// `[elt for ...]`, a new list with the value of `elt` for each turn of
    /// the loops.
    pub(super) fn list_comp(
        &self,
        expr: &ast::Expr,
        comp: &ast::ExprListComp,
    ) -> Result<Expr, Error> {
        let rep = self.rep_of(expr_shape(self.model(), expr));
        let Some(list) = rep.and_then(|rep| self.list_class(rep)) else {
            return Err(unsupported(expr, "a list of this type"));
        };
        let rep = Rep::Object(list);
        let item = self.list_item_of(rep).expect("a list");
        let result = self.hidden_var(rep);
        let read = || Expr::Load {
            place: result,
            rep,
            check: None,
        };
        let mut stmts = vec![Stmt::Assign {
            targets: vec![result],
            value: self.list_of_items(list, &[], self.site(expr.range()))?,
        }];
        stmts.extend(self.comprehension(&comp.generators, &mut || {
            let value = self.literal_item(&comp.elt, item)?;
            let append = self.list_call(expr, "append", vec![read(), value])?;
            Ok(vec![Stmt::Expr(append)])
        })?);
        Ok(Expr::Block {
            stmts,
            value: Box::new(read()),
        })
    }

    /// `{elt for ...}`, a new `set` that the method `add` of the copy of
    /// the class `Set` of the prelude fills, one value at a time, as
    /// `SET_ADD` does. The traceback marks `elt` for an error of `add`.
    pub(super) fn set_comp(
        &self,
        expr: &ast::Expr,
        comp: &ast::ExprSetComp,
    ) -> Result<Expr, Error> {
        let class = self
            .copy_of(expr)
            .ok_or_else(|| unsupported(expr, "a set of this type"))?;
        let add = self.method_func(class, "add").expect("Set has add");
        let params = self.funcs.borrow().get(add).params.clone();
        let [_, item] = params[..] else {
            unreachable!("add takes a key");
        };
        let site = self.site(comp.elt.range());
        self.filled(expr, class, &comp.generators, &mut |object| {
            let value = self.literal_item(&comp.elt, item)?;
            Ok(generated_call(add, vec![object, value], Rep::None, site))
        })
    }

    /// `{key: value for ...}`, a new `dict` that the method `__setitem__`
    /// of the copy of the class `Dict` of the prelude fills, as `MAP_ADD`
    /// does. The key runs before the value, and the traceback marks both
    /// for an error of `__setitem__`.
    pub(super) fn dict_comp(
        &self,
        expr: &ast::Expr,
        comp: &ast::ExprDictComp,
    ) -> Result<Expr, Error> {
        let Some(key) = comp.key.as_deref() else {
            return Err(unsupported(&*comp.value, "a starred item"));
        };
        let class = self
            .copy_of(expr)
            .ok_or_else(|| unsupported(expr, "a dict of this type"))?;
        let set = self
            .method_func(class, "__setitem__")
            .expect("Dict has __setitem__");
        let params = self.funcs.borrow().get(set).params.clone();
        let [_, key_rep, value_rep] = params[..] else {
            unreachable!("__setitem__ takes a key and a value");
        };
        let site = self.site(TextRange::new(key.start(), comp.value.end()));
        self.filled(expr, class, &comp.generators, &mut |object| {
            let key = self.literal_item(key, key_rep)?;
            let value = self.literal_item(&comp.value, value_rep)?;
            Ok(generated_call(
                set,
                vec![object, key, value],
                Rep::None,
                site,
            ))
        })
    }

    /// A new object of `class`, a copy of a class of the prelude, with
    /// the call that `put` gives for the object in each turn of the loops of
    /// `generators`.
    fn filled(
        &self,
        expr: &ast::Expr,
        class: ClassId,
        generators: &[ast::Comprehension],
        put: &mut dyn FnMut(Expr) -> Result<Expr, Error>,
    ) -> Result<Expr, Error> {
        let rep = Rep::Object(class);
        let result = self.hidden_var(rep);
        let read = || Expr::Load {
            place: result,
            rep,
            check: None,
        };
        let mut stmts = vec![Stmt::Assign {
            targets: vec![result],
            value: self.new_copy(class, Vec::new(), self.site(expr.range())),
        }];
        stmts.extend(self.comprehension(generators, &mut || Ok(vec![Stmt::Expr(put(read())?)]))?);
        Ok(Expr::Block {
            stmts,
            value: Box::new(read()),
        })
    }

    /// The loops of `generators`, one inside the other, with the code that
    /// `body` gives inside the last one. The first iterable runs before the
    /// comprehension binds any name, and the tests of a loop run before the
    /// loop inside it.
    pub(super) fn comprehension(
        &self,
        generators: &[ast::Comprehension],
        body: &mut dyn FnMut() -> Result<Vec<Stmt>, Error>,
    ) -> Result<Vec<Stmt>, Error> {
        let mark = self.with_comprehension(|comp| {
            comp.open += 1;
            comp.vars.len()
        });
        let loops = self.clauses(generators, body);
        self.with_comprehension(|comp| {
            comp.open -= 1;
            comp.vars.truncate(mark);
        });
        loops
    }

    pub(super) fn clauses(
        &self,
        generators: &[ast::Comprehension],
        body: &mut dyn FnMut() -> Result<Vec<Stmt>, Error>,
    ) -> Result<Vec<Stmt>, Error> {
        let Some((first, rest)) = generators.split_first() else {
            return body();
        };
        if first.is_async {
            return Err(unsupported(first, "an async comprehension"));
        }
        self.for_loop(
            &first.target,
            &first.iter,
            &mut || {
                let tests = first
                    .ifs
                    .iter()
                    .map(|test| self.truth_of(test))
                    .collect::<Result<Vec<_>, _>>()?;
                let inner = self.clauses(rest, body)?;
                Ok(tests.into_iter().rev().fold(inner, |inner, cond| {
                    vec![Stmt::If {
                        cond,
                        then: inner,
                        else_: Vec::new(),
                    }]
                }))
            },
            &[],
        )
    }

    /// `f` of the comprehensions around the code of the function, or of the
    /// module.
    pub(super) fn with_comprehension<R>(&self, f: impl FnOnce(&mut Comprehension) -> R) -> R {
        match self.scope.borrow_mut().as_mut() {
            Some(scope) => f(&mut scope.comprehension),
            None => f(&mut self.comprehension.borrow_mut()),
        }
    }

    /// The variable and the rep of `name` when a comprehension around the
    /// code binds it, the innermost one.
    pub(super) fn comprehension_var(&self, name: &str) -> Option<(Place, Rep)> {
        self.with_comprehension(|comp| {
            comp.vars
                .iter()
                .rev()
                .find(|(var, _, _)| var == name)
                .map(|(_, place, rep)| (*place, *rep))
        })
    }

    /// A new variable of `rep` for `name`, the target of a loop of a
    /// comprehension, and `None` outside a comprehension.
    pub(super) fn comprehension_target(&self, name: &str, rep: Rep) -> Option<Place> {
        if self.with_comprehension(|comp| comp.open) == 0 {
            return None;
        }
        let place = self.hidden_var(rep);
        self.with_comprehension(|comp| comp.vars.push((name.to_owned(), place, rep)));
        Some(place)
    }

    /// Returns `true` if a comprehension around the code binds one of
    /// `names`, `false` otherwise.
    pub(super) fn binds_any<'n>(&self, mut names: impl Iterator<Item = &'n str>) -> bool {
        names.any(|name| self.comprehension_var(name).is_some())
    }
}
