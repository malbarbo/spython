//! The generator expressions that a consumer takes, as `sum(x for x in xs)`.
//! The code of a generator is a function with the frame `<genexpr>`, which
//! takes the first iterable, as in CPython, where the code around it runs
//! the iterable. The consumer runs in the frame of the code around it too.

use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::capture::free_names_of_generator;
use super::flow::names_in;
use super::{Lowering, Scope, ScopeCapture, Vars, read, unsupported};
use crate::Error;
use crate::ir::{Captured, Expr, Place, Rep, Stmt};

/// What the function of a generator does with each value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Consume {
    /// Puts the values in a new list, which the consumer takes. CPython
    /// makes the list too for `join` and `sorted`, but `sum`, `min` and
    /// `max` take the values as they come.
    List,
    /// Gives `True` at the first value that is true, for `any`.
    Any,
    /// Gives `False` at the first value that is false, for `all`.
    All,
}

impl Lowering<'_, '_> {
    /// The value of `arg`, the argument of the consumer `call`, with a
    /// generator expression as the list of its values.
    pub(super) fn consumed_list(
        &self,
        call: &ast::ExprCall,
        arg: &ast::Expr,
    ) -> Result<Expr, Error> {
        match arg {
            ast::Expr::Generator(generator) => self.generator(call, generator, Consume::List),
            arg => self.expr(arg),
        }
    }

    /// The call at `call` of the function of `generator`, which gives what
    /// `consume` asks.
    pub(super) fn generator(
        &self,
        call: &ast::ExprCall,
        generator: &ast::ExprGenerator,
        consume: Consume,
    ) -> Result<Expr, Error> {
        let (stmts, value) = self.generator_call(call, generator, consume)?;
        Ok(Expr::Block {
            stmts,
            value: Box::new(value),
        })
    }

    /// [`Self::generator`] as the statements that run the parts of the
    /// first iterable, in order, and the call that runs the generator.
    pub(super) fn generator_call(
        &self,
        call: &ast::ExprCall,
        generator: &ast::ExprGenerator,
        consume: Consume,
    ) -> Result<(Vec<Stmt>, Expr), Error> {
        self.check_new_frame(generator, "a generator expression")?;
        let Some(first) = generator.generators.first() else {
            return Err(unsupported(generator, "this generator expression"));
        };
        if first.is_async {
            return Err(unsupported(first, "an async comprehension"));
        }
        let ret = match consume {
            Consume::List => {
                let shape = expr_shape(self.model(), &generator.elt)
                    .ok_or_else(|| unsupported(&*generator.elt, "a value of this type"))?;
                self.rep_of(Some(Shape::List(Box::new(shape))))
                    .ok_or_else(|| unsupported(&*generator.elt, "a list of this type"))?
            }
            Consume::Any | Consume::All => Rep::Bool,
        };

        // The names that the loops bind are the locals of the frame, after
        // `.0`, the parameter that takes the first iterable.
        let mut stmts = Vec::new();
        let mut operands = Vec::new();
        for (node, value) in self.generator_operands(&first.iter)? {
            let held = self.hold(&mut stmts, value);
            operands.push((node, read(held)));
        }
        let mut operands = operands.into_iter();
        let (first_node, first_arg) = operands.next().expect("an iterable with a part");
        let first_rep = first_arg.rep();
        let mut vars = Vars::default();
        vars.add(".0", first_rep);
        let mut order = vec![".0".to_owned()];
        for clause in &generator.generators {
            for name in names_in(&clause.target) {
                let id = name.id.as_str();
                if order.iter().any(|known| known == id) {
                    continue;
                }
                let rep = self
                    .rep_of(expr_shape(self.model(), &ast::Expr::Name(name.clone())))
                    .ok_or_else(|| unsupported(name, "a variable of this type"))?;
                vars.add(id, rep);
                order.push(id.to_owned());
            }
        }
        let (param, _) = vars.find(".0").expect("the parameter .0");
        let mut substitutes = vec![(
            first_node,
            Expr::Load {
                place: Place::Local(param),
                rep: first_rep,
                check: None,
            },
        )];
        // The code around the generator runs the other parts of the first
        // iterable too, and the function captures them.
        let (mut captured, mut captures) = self.generator_captures(generator)?;
        for (node, value) in operands {
            let index = u32::try_from(captures.len()).expect("fewer than 2^32 captures");
            let rep = value.rep();
            captured.push(Captured::Value(value));
            // A capture without a name holds a part of the first iterable.
            captures.push(ScopeCapture {
                name: String::new(),
                rep,
                cell: false,
                unbound: false,
            });
            substitutes.push((
                node,
                Expr::Capture {
                    index,
                    rep,
                    check: None,
                },
            ));
        }
        let scope = Scope::function("<genexpr>".to_owned(), 1, ret, vars, order, captures);
        let ty = self.func_type(vec![first_rep], ret);
        let (scope, body) = self.in_scope(scope, || {
            *self.substitutes.borrow_mut() = substitutes;
            let body = self.generator_body(generator, consume, ret);
            self.substitutes.borrow_mut().clear();
            body
        });
        let code = self.define_code(ty, scope, body?);
        let call = Expr::CallValue {
            callee: Box::new(Expr::Closure {
                func: code,
                ty,
                captures: Some(captured),
            }),
            args: vec![first_arg],
            rep: ret,
            site: self.site(call.range()),
            // CPython makes the generator in a frame of its own, before the
            // consumer runs it, and the frame raises at the generator, with
            // the parentheses of the call when the generator has none.
            limit: self.site(if generator.parenthesized {
                generator.range()
            } else {
                call.arguments.range()
            }),
        };
        Ok((stmts, call))
    }

    /// The code of the function of `generator`, whose result has `ret`.
    fn generator_body(
        &self,
        generator: &ast::ExprGenerator,
        consume: Consume,
        ret: Rep,
    ) -> Result<Vec<Stmt>, Error> {
        let result = self.hidden_var(ret);
        let read = || Expr::Load {
            place: result,
            rep: ret,
            check: None,
        };
        let mut stmts = Vec::new();
        let last = match (consume, ret) {
            (Consume::List, Rep::List(list)) => {
                stmts.push(Stmt::Assign {
                    targets: vec![result],
                    value: Expr::List {
                        list,
                        items: Vec::new(),
                    },
                });
                read()
            }
            (Consume::Any, _) => Expr::Bool(false),
            _ => Expr::Bool(true),
        };
        stmts.extend(self.clauses(&generator.generators, &mut || {
            let put = match (consume, ret) {
                (Consume::List, Rep::List(list)) => {
                    let value = self.literal_item(&generator.elt, self.list_item(list))?;
                    Stmt::Expr(self.list_call(&*generator.elt, "append", vec![read(), value])?)
                }
                (Consume::Any, _) => Stmt::If {
                    cond: self.truth_of(&generator.elt)?,
                    then: vec![Stmt::Return(Some(Expr::Bool(true)))],
                    else_: Vec::new(),
                },
                _ => Stmt::If {
                    cond: Expr::Not(Box::new(self.truth_of(&generator.elt)?)),
                    then: vec![Stmt::Return(Some(Expr::Bool(false)))],
                    else_: Vec::new(),
                },
            };
            Ok(vec![put])
        })?);
        stmts.push(Stmt::Return(Some(last)));
        Ok(stmts)
    }

    /// The parts of `iter` that a loop runs before it starts, each one
    /// lowered in the code around the generator, with its node.
    fn generator_operands(&self, iter: &ast::Expr) -> Result<Vec<(TextRange, Expr)>, Error> {
        if let Some(range) = self.range_call(iter) {
            let operands = range
                .arguments
                .args
                .iter()
                .map(|arg| Ok((arg.range(), self.int_operand(arg)?)))
                .collect::<Result<Vec<_>, Error>>()?;
            // A step of 0 raises in the frame of `range`, where the loop of
            // the generator would raise it in its own frame.
            if let [_, _, (_, step)] = &*operands
                && !matches!(step, Expr::Int(step) if *step != 0)
            {
                return Err(unsupported(
                    range,
                    "a generator expression over a range with this step",
                ));
            }
            return Ok(operands);
        }
        let node = self.dict_loop(iter).map_or(iter, |(dict, _)| dict);
        Ok(vec![(node.range(), self.expr(node)?)])
    }

    /// What a new value of the function of `generator` holds for each name
    /// that it reads of the code around it. A name of a comprehension
    /// around the generator goes as a copy, since the consumer takes every
    /// value before the name changes.
    fn generator_captures(
        &self,
        generator: &ast::ExprGenerator,
    ) -> Result<(Vec<Captured>, Vec<ScopeCapture>), Error> {
        let free = free_names_of_generator(generator);
        let (outer, others): (Vec<&str>, Vec<&str>) = free
            .into_iter()
            .partition(|name| self.comprehension_var(name).is_some());
        let (mut captured, mut captures) =
            self.closure_captures(generator, generator.range(), others)?;
        for name in outer {
            let (place, rep) = self
                .comprehension_var(name)
                .expect("a name of a comprehension");
            captured.push(Captured::Value(Expr::Load {
                place,
                rep,
                check: None,
            }));
            captures.push(ScopeCapture {
                name: name.to_owned(),
                rep,
                cell: false,
                unbound: false,
            });
        }
        Ok((captured, captures))
    }

    /// The value of the part of the first iterable at `node`, which the
    /// code around the generator runs, and `None` for another node.
    pub(super) fn substitute(&self, node: &impl Ranged) -> Option<Expr> {
        let mut substitutes = self.substitutes.borrow_mut();
        let i = substitutes
            .iter()
            .position(|(range, _)| *range == node.range())?;
        Some(substitutes.swap_remove(i).1)
    }
}
