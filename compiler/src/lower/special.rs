//! The operators and the builtins that call a special method of a class of
//! the program.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::class::{generated_call, param, widen};
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{ClassId, CmpOp, Expr, FuncId, Rep, Site, Stmt};

/// A lowered operand, with the type that the checker gives it.
pub(super) type Operand = (Expr, Option<Shape>);

impl Lowering<'_, '_> {
    /// The method of `class` that `left op right` calls with an object of
    /// the class on the left, for `op` `==` or `!=`, and `true` when the
    /// comparison is the `not` of its result. `!=` calls `__ne__`, and the
    /// `__ne__` of `object` gives the `not` of `__eq__`. `None` when the
    /// method is the one of `object`, which compares the identity of the
    /// objects.
    fn eq_method(&self, class: ClassId, op: CmpOp) -> Option<(FuncId, bool)> {
        let method = |name| self.method(class, name).and_then(|method| method.func);
        match op {
            CmpOp::Ne => method("__ne__")
                .map(|func| (func, false))
                .or_else(|| method("__eq__").map(|func| (func, true))),
            _ => method("__eq__").map(|func| (func, false)),
        }
    }

    /// The class of each object that a value of `rep` may be, where `shape`
    /// gives the type of a uniform value.
    fn classes_of(&self, rep: Rep, shape: impl FnOnce() -> Option<Shape>) -> Vec<ClassId> {
        match rep {
            Rep::Object(class) | Rep::ObjectOrNone(class) => self.descendants(class),
            Rep::Uniform => self.classes_in(shape()),
            _ => Vec::new(),
        }
    }

    /// Returns `true` if `left op right` of two lowered operands, each with
    /// its node, may call a special method of the program, `false`
    /// otherwise. An order calls one when an operand is an object, and `==`
    /// and `!=` when an operand may be an object of a class with `__eq__`
    /// or `__ne__`.
    pub(super) fn calls_special(
        &self,
        (left, left_node): &(Expr, &ast::Expr),
        (right, right_node): &(Expr, &ast::Expr),
        op: CmpOp,
    ) -> bool {
        if order_methods(op).is_some() {
            return left.rep().class().is_some() || right.rep().class().is_some();
        }
        let shape = |node| move || expr_shape(self.model(), node);
        self.calls_eq(
            (left.rep(), shape(left_node)),
            (right.rep(), shape(right_node)),
            op,
        )
    }

    /// Returns `true` if `left op right` of values of the reps of `left`
    /// and `right`, each with the function of its type, is `==` or `!=` and
    /// may call a method `__eq__` or `__ne__` of the program, `false`
    /// otherwise.
    pub(super) fn calls_eq(
        &self,
        left: (Rep, impl FnOnce() -> Option<Shape>),
        right: (Rep, impl FnOnce() -> Option<Shape>),
        op: CmpOp,
    ) -> bool {
        matches!(op, CmpOp::Eq | CmpOp::Ne)
            && self
                .classes_of(left.0, left.1)
                .into_iter()
                .chain(self.classes_of(right.0, right.1))
                .any(|class| self.eq_method(class, op).is_some())
    }

    /// The comparison `c`, with `ops` and the lowered `operands`, where a
    /// pair of operands may call a special method. A chain compares each
    /// pair of operands, and stops at the first pair that is not true.
    pub(super) fn special_compare(
        &self,
        c: &ast::ExprCompare,
        ops: Vec<CmpOp>,
        operands: Vec<(Expr, &ast::Expr)>,
    ) -> Result<Expr, Error> {
        let site = self.site(c.range());
        // A value of `None` has no local of its own.
        let operands: Vec<Operand> = operands
            .into_iter()
            .map(|(lowered, node)| {
                let lowered = match lowered.rep() {
                    Rep::None => self
                        .coerce(lowered, Rep::Uniform)
                        .expect("a uniform value holds None"),
                    _ => lowered,
                };
                (lowered, expr_shape(self.model(), node))
            })
            .collect();
        let mut operands = operands.into_iter();
        let (first, mut shape) = operands.next().expect("a comparison has a first operand");
        if let [op] = *ops {
            let right = operands.next().expect("a comparison has a second operand");
            return self.pair_test(c, op, (first, shape), right, site);
        }
        let mut links = Vec::new();
        let mut rep = first.rep();
        for (op, (operand, operand_shape)) in ops.into_iter().zip(operands) {
            let left = (Expr::ChainLeft(rep), shape);
            let right = (Expr::ChainRight(operand.rep()), operand_shape.clone());
            rep = operand.rep();
            shape = operand_shape;
            links.push((operand, self.pair_test(c, op, left, right, site)?));
        }
        Ok(Expr::Chain {
            first: Box::new(first),
            links,
        })
    }

    /// `left op right` of a comparison `c` at `site`, a `bool`, where an
    /// operand is an object.
    fn pair_test(
        &self,
        c: &ast::ExprCompare,
        op: CmpOp,
        left: Operand,
        right: Operand,
        site: Site,
    ) -> Result<Expr, Error> {
        let object = left.0.rep().class().is_some() || right.0.rep().class().is_some();
        if order_methods(op).is_some() && object {
            return self.special_order(c, op, left, right, site);
        }
        if self.calls_eq(
            (left.0.rep(), || left.1.clone()),
            (right.0.rep(), || right.1.clone()),
            op,
        ) {
            return self.special_eq(c, op, left, right, site);
        }
        if order_methods(op).is_some() || !object {
            return Err(unsupported(
                c,
                "a chain of comparisons of objects and other values",
            ));
        }
        let eq = self.object_eq(left, right, site)?;
        Ok(match op {
            CmpOp::Ne => Expr::Not(Box::new(eq)),
            _ => eq,
        })
    }

    /// `left op right` for an order `op` of a comparison `c` at `site`, as
    /// CPython finds it. The method of the left object comes first, as
    /// `__lt__` for `<`, and the reflected method of the right object comes
    /// when the left one has none, as `__gt__` for `<`. The checker proves
    /// that one of them has the method.
    fn special_order(
        &self,
        c: &ast::ExprCompare,
        op: CmpOp,
        left: Operand,
        right: Operand,
        site: Site,
    ) -> Result<Expr, Error> {
        let (name, reflected) = order_methods(op).expect("an order");
        let reps = [left.0.rep(), right.0.rep()];
        if reps
            .iter()
            .any(|rep| matches!(rep, Rep::Uniform | Rep::ObjectOrNone(_)))
        {
            return Err(unsupported(c, "an order of objects of a union"));
        }
        let method = |class, name| self.method(class, name).and_then(|method| method.func);
        let lefts = self.classes_of(left.0.rep(), || left.1.clone());
        let rights = self.classes_of(right.0.rep(), || right.1.clone());
        // CPython calls the reflected method of the right object first when
        // its class descends from the class of the left object.
        let first = rights.iter().any(|right| {
            method(*right, reflected).is_some()
                && lefts
                    .iter()
                    .any(|left| left != right && self.descends(*right, *left))
        });
        if first {
            return Err(unsupported(
                c,
                "a comparison with an object of a subclass on the right",
            ));
        }
        if let Rep::Object(class) = left.0.rep()
            && let Some(func) = method(class, name)
            && lefts.iter().all(|left| method(*left, name) == Some(func))
        {
            return self.call_order(c, func, left.0, right.0, site);
        }
        let operand = |i| param(i, if i == 0 { reps[0] } else { reps[1] });
        let mut body = Vec::new();
        for (i, classes, name) in [(0, &lefts, name), (1, &rights, reflected)] {
            for class in classes {
                let Some(func) = method(*class, name) else {
                    continue;
                };
                let exact = Expr::AsExact {
                    value: Box::new(operand(i)),
                    class: *class,
                };
                let call = self.call_order(c, func, exact, operand(1 - i), Site::NONE)?;
                body.push(Stmt::If {
                    cond: Expr::IsExact {
                        value: Box::new(operand(i)),
                        class: *class,
                    },
                    then: vec![Stmt::Return(Some(call))],
                    else_: Vec::new(),
                });
            }
        }
        body.push(Stmt::Unreachable);
        let func = self.generated(reps.to_vec(), Rep::Bool, body);
        Ok(generated_call(func, vec![left.0, right.0], Rep::Bool, site))
    }

    /// The call of `func`, a method of an order, with `object` as `self`
    /// and `other` as the other operand.
    fn call_order(
        &self,
        c: &ast::ExprCompare,
        func: FuncId,
        object: Expr,
        other: Expr,
        site: Site,
    ) -> Result<Expr, Error> {
        let params = self.funcs.borrow().get(func).params.clone();
        let (Some(receiver), Some(param)) = (params.first(), params.get(1)) else {
            unreachable!("a method of an order has self and other");
        };
        let other = self
            .coerce(other, *param)
            .ok_or_else(|| unsupported(c, "an order with an operand of this type"))?;
        Ok(generated_call(
            func,
            vec![widen(object, *receiver), other],
            Rep::Bool,
            site,
        ))
    }

    /// `left op right`, for `op` `==` or `!=`, at `site`, as CPython finds
    /// it. The method of the left object comes first, and the method of
    /// the right object comes when the left one is the one of `object`.
    /// Without either, the objects are equal when they are the same object,
    /// or objects of the same dataclass with equal fields.
    pub(super) fn special_eq(
        &self,
        node: &impl Ranged,
        op: CmpOp,
        left: Operand,
        right: Operand,
        site: Site,
    ) -> Result<Expr, Error> {
        let lefts = self.classes_of(left.0.rep(), || left.1.clone());
        let rights = self.classes_of(right.0.rep(), || right.1.clone());
        // CPython calls the method of the right object first when its class
        // descends from the class of the left object.
        let reflected = rights.iter().any(|right| {
            self.eq_method(*right, op).is_some()
                && lefts
                    .iter()
                    .any(|left| left != right && self.descends(*right, *left))
        });
        if reflected {
            return Err(unsupported(
                node,
                "a comparison with an object of a subclass on the right",
            ));
        }
        // An object of the left class decides when every class that it may
        // be has the same method.
        if let Rep::Object(class) = left.0.rep()
            && let Some(method) = self.eq_method(class, op)
            && lefts
                .iter()
                .all(|left| self.eq_method(*left, op) == Some(method))
        {
            return Ok(self.call_eq(method, left.0, right.0, site));
        }
        let (left_rep, right_rep) = (left.0.rep(), right.0.rep());
        let operand = |i| param(i, if i == 0 { left_rep } else { right_rep });
        let mut body = Vec::new();
        for (i, classes) in [(0, &lefts), (1, &rights)] {
            let value = || operand(i);
            let other = || operand(1 - i);
            for class in classes {
                let Some(method) = self.eq_method(*class, op) else {
                    continue;
                };
                let exact = Expr::AsExact {
                    value: Box::new(value()),
                    class: *class,
                };
                body.push(Stmt::If {
                    cond: Expr::IsExact {
                        value: Box::new(value()),
                        class: *class,
                    },
                    then: vec![Stmt::Return(Some(self.call_eq(
                        method,
                        exact,
                        other(),
                        Site::NONE,
                    )))],
                    else_: Vec::new(),
                });
            }
        }
        let eq = self.object_eq(
            (param(0, left_rep), left.1),
            (param(1, right_rep), right.1),
            Site::NONE,
        )?;
        body.push(Stmt::Return(Some(match op {
            CmpOp::Ne => Expr::Not(Box::new(eq)),
            _ => eq,
        })));
        let func = self.generated(vec![left_rep, right_rep], Rep::Bool, body);
        Ok(generated_call(func, vec![left.0, right.0], Rep::Bool, site))
    }

    /// The call of `method` of `special_eq` with `object` as `self` and
    /// `other` as the uniform value of the other operand.
    fn call_eq(&self, (func, not): (FuncId, bool), object: Expr, other: Expr, site: Site) -> Expr {
        let receiver = *self
            .funcs
            .borrow()
            .get(func)
            .params
            .first()
            .expect("a method has self");
        let other = self
            .coerce(other, Rep::Uniform)
            .expect("a uniform value holds any value");
        let call = generated_call(func, vec![widen(object, receiver), other], Rep::Bool, site);
        if not { Expr::Not(Box::new(call)) } else { call }
    }

    /// `left == right` of two values that are not objects of a class with
    /// `__eq__`. Two objects are equal when they are the same object, or
    /// objects of the same dataclass with equal fields.
    fn object_eq(&self, left: Operand, right: Operand, site: Site) -> Result<Expr, Error> {
        let theirs = self.dataclasses_in(right.0.rep(), || right.1.clone());
        let mut shared = self.dataclasses_in(left.0.rep(), || left.1.clone());
        shared.retain(|class| theirs.contains(class));
        if !shared.is_empty() {
            return self.dataclass_eq(left.0, right.0, shared, site);
        }
        let rep = self.join(left.0.rep(), right.0.rep());
        let coerce = |value| self.coerce(value, rep).expect("the join holds both values");
        Ok(Expr::Compare(
            Box::new(coerce(left.0)),
            vec![(CmpOp::Eq, coerce(right.0))],
        ))
    }
}

/// The method that `op` calls on the left object and the reflected one
/// that it calls on the right object, and `None` when `op` is no order.
fn order_methods(op: CmpOp) -> Option<(&'static str, &'static str)> {
    match op {
        CmpOp::Lt => Some(("__lt__", "__gt__")),
        CmpOp::Le => Some(("__le__", "__ge__")),
        CmpOp::Gt => Some(("__gt__", "__lt__")),
        CmpOp::Ge => Some(("__ge__", "__le__")),
        CmpOp::Eq | CmpOp::Ne => None,
    }
}
