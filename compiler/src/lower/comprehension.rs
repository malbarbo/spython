//! The comprehensions, which run as a loop in line, as CPython 3.12 makes
//! them (PEP 709). The names of a comprehension are variables of its own,
//! and the traceback has no frame for it.

use ruff_python_ast as ast;
use ty_python_semantic::types::lowering::expr_shape;

use super::{Comprehension, Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, Place, Rep, Stmt};

impl<'a> Lowering<'a, '_> {
    /// `[elt for ...]`, a new list with the value of `elt` for each turn of
    /// the loops.
    pub(super) fn list_comp(
        &self,
        expr: &ast::Expr,
        comp: &ast::ExprListComp,
    ) -> Result<Expr, Error> {
        let Some(Rep::List(list)) = self.rep_of(expr_shape(self.model(), expr)) else {
            return Err(unsupported(expr, "a list of this type"));
        };
        let rep = Rep::List(list);
        let item = self.list_item(list);
        let result = self.hidden_var(rep);
        let read = || Expr::Load {
            place: result,
            rep,
            check: None,
        };
        let mut stmts = vec![Stmt::Assign {
            targets: vec![result],
            value: Expr::List {
                list,
                items: Vec::new(),
            },
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

    fn clauses(
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
