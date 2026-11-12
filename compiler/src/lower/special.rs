//! The operators and the builtins that call a special method of a class of
//! the program.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::class::{generated_call, param, widen};
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{ClassId, CmpOp, Expr, FuncId, LogicOp, Rep, Site, Stmt};

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
            return self.special_eq(op, left, right, site);
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
    /// CPython finds it. The reflected method of the right object, as
    /// `__gt__` for `<`, comes first when its class descends from the class
    /// of the left object. Then comes the method of the left object, as
    /// `__lt__` for `<`, and the reflected method of the right object when
    /// the left one has none. The checker proves that one of them has the
    /// method.
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
        let method = |class, name| {
            self.method(class, name)
                .and_then(|method| method.func)
                .map(|func| (func, false))
        };
        self.dispatch_binary(
            (left, right, Rep::Bool, site),
            &|class| method(class, name),
            &|class| method(class, reflected),
            &|_, _| true,
            &|(func, _), object, other, site| self.call_operator(c, func, object, other, site),
            &|_, _| Ok(vec![Stmt::Unreachable]),
        )
    }

    /// `left op right`, for `op` `==` or `!=`, at `site`, as CPython finds
    /// it. The method of the right object comes first when its class
    /// descends from the class of the left object. Then comes the method of
    /// the left object, and the method of the right object when the left
    /// one is the one of `object`. Without either, the objects are equal
    /// when they are the same object, or objects of the same dataclass with
    /// equal fields.
    pub(super) fn special_eq(
        &self,
        op: CmpOp,
        left: Operand,
        right: Operand,
        site: Site,
    ) -> Result<Expr, Error> {
        let (left_shape, right_shape) = (left.1.clone(), right.1.clone());
        self.dispatch_binary(
            (left, right, Rep::Bool, site),
            &|class| self.eq_method(class, op),
            &|class| self.eq_method(class, op),
            &|_, _| true,
            &|method, object, other, site| Ok(self.call_eq(method, object, other, site)),
            &|v, w| {
                let eq = self.object_eq(
                    (v, left_shape.clone()),
                    (w, right_shape.clone()),
                    Site::NONE,
                )?;
                Ok(vec![Stmt::Return(Some(match op {
                    CmpOp::Ne => Expr::Not(Box::new(eq)),
                    _ => eq,
                }))])
            },
        )
    }

    /// `left op right` of an arithmetic operator at `node`, where an
    /// operand is an object, as CPython finds it. The reflected method of
    /// the right object, as `__radd__` for `+`, comes first when its class
    /// descends from the class of the left object and has a reflected
    /// method other than the one of the left class. Then comes the method
    /// of the left object, as `__add__`, and then the reflected method of
    /// the right object. The checker proves that one of them has the
    /// method.
    pub(super) fn special_arith(
        &self,
        node: &impl Ranged,
        op: ast::Operator,
        left: Expr,
        right: Expr,
    ) -> Result<Expr, Error> {
        let (name, reflected) =
            arith_methods(op).ok_or_else(|| unsupported(node, "this operator on an object"))?;
        if [left.rep(), right.rep()].iter().any(|rep| {
            !matches!(rep, Rep::Object(_)) && rep.class().is_some() || *rep == Rep::Uniform
        }) {
            return Err(unsupported(node, "an operator on an object of a union"));
        }
        let method = |class, name| {
            self.method(class, name)
                .and_then(|method| method.func)
                .map(|func| (func, false))
        };
        let lefts = self.classes_of(left.rep(), || None);
        let rights = self.classes_of(right.rep(), || None);
        let ret = lefts
            .iter()
            .filter_map(|class| method(*class, name))
            .chain(rights.iter().filter_map(|class| method(*class, reflected)))
            .map(|(func, _)| self.funcs.borrow().get(func).ret)
            .reduce(|a, b| self.join(a, b))
            .ok_or_else(|| unsupported(node, "this operator on an object"))?;
        let site = self.site(node.range());
        self.dispatch_binary(
            ((left, None), (right, None), ret, site),
            &|class| method(class, name),
            &|class| method(class, reflected),
            &|left, right| method(right, reflected) != method(left, reflected),
            &|(func, _), object, other, site| {
                let call = self.call_operator(node, func, object, other, site)?;
                Ok(self.coerce(call, ret).expect("the join holds every result"))
            },
            &|_, _| Ok(vec![Stmt::Unreachable]),
        )
    }

    /// `-operand` of `u`, an object, which calls `__neg__` of its class.
    pub(super) fn special_neg(&self, u: &ast::ExprUnaryOp, operand: Expr) -> Result<Expr, Error> {
        let Rep::Object(class) = operand.rep() else {
            return Err(unsupported(u, "an operator on an object of a union"));
        };
        let method = self
            .method(class, "__neg__")
            .ok_or_else(|| unsupported(u, "this operator on an object"))?;
        let func = self.method_slot(class, method);
        let ret = self.funcs.borrow().get(func).ret;
        Ok(generated_call(
            func,
            vec![operand],
            ret,
            self.site(u.range()),
        ))
    }

    /// The call of `func`, the method of an operator at `node`, with
    /// `object` as `self` and `other` as the other operand.
    fn call_operator(
        &self,
        node: &impl Ranged,
        func: FuncId,
        object: Expr,
        other: Expr,
        site: Site,
    ) -> Result<Expr, Error> {
        let sig = self.funcs.borrow().get(func).clone();
        let (Some(receiver), Some(param)) = (sig.params.first(), sig.params.get(1)) else {
            unreachable!("a method of an operator has self and other");
        };
        let other = self
            .coerce(other, *param)
            .ok_or_else(|| unsupported(node, "an operator with an operand of this type"))?;
        Ok(generated_call(
            func,
            vec![widen(object, *receiver), other],
            sig.ret,
            site,
        ))
    }

    /// The value of a binary operator on `left` and `right`, a value of
    /// `ret` at `site`, that calls a special method as CPython finds it.
    /// `direct` gives the method that the operator calls on an object of a
    /// class on the left, and `reflected` the one on the right, each with
    /// whether the value is the `not` of the result of the method. The
    /// reflected method of the right object comes first when its class is
    /// a proper subclass of the class of the left object and `first` takes
    /// the two classes. Then comes the method of the left object, and then
    /// the reflected method of the right object. `call` calls a method with
    /// an object and the other operand, and `rest` gives the code that runs
    /// when no object has a method, with the two operands.
    fn dispatch_binary(
        &self,
        (left, right, ret, site): (Operand, Operand, Rep, Site),
        direct: &dyn Fn(ClassId) -> Option<(FuncId, bool)>,
        reflected: &dyn Fn(ClassId) -> Option<(FuncId, bool)>,
        first: &dyn Fn(ClassId, ClassId) -> bool,
        call: &dyn Fn((FuncId, bool), Expr, Expr, Site) -> Result<Expr, Error>,
        rest: &dyn Fn(Expr, Expr) -> Result<Vec<Stmt>, Error>,
    ) -> Result<Expr, Error> {
        let lefts = self.classes_of(left.0.rep(), || left.1.clone());
        let rights = self.classes_of(right.0.rep(), || right.1.clone());
        // The classes on the left whose objects give the call to the
        // reflected method of an object of each class on the right.
        let firsts: Vec<(ClassId, Vec<ClassId>)> = rights
            .iter()
            .filter(|right| reflected(**right).is_some())
            .map(|right| {
                let ancestors = lefts
                    .iter()
                    .copied()
                    .filter(|left| {
                        left != right && self.descends(*right, *left) && first(*left, *right)
                    })
                    .collect();
                (*right, ancestors)
            })
            .filter(|(_, ancestors): &(_, Vec<_>)| !ancestors.is_empty())
            .collect();
        // An object of the left class decides when every class that it may
        // be has the same method.
        if firsts.is_empty()
            && let Rep::Object(class) = left.0.rep()
            && let Some(method) = direct(class)
            && lefts.iter().all(|left| direct(*left) == Some(method))
        {
            return call(method, left.0, right.0, site);
        }
        let reps = [left.0.rep(), right.0.rep()];
        let operand = |i| param(i, if i == 0 { reps[0] } else { reps[1] });
        let is_exact = |i, class| Expr::IsExact {
            value: Box::new(operand(i)),
            class,
        };
        let exact = |i, class| Expr::AsExact {
            value: Box::new(operand(i)),
            class,
        };
        let mut body = Vec::new();
        let arm = |cond, i, class, method| {
            Ok::<_, Error>(Stmt::If {
                cond,
                then: vec![Stmt::Return(Some(call(
                    method,
                    exact(i, class),
                    operand(1 - i),
                    Site::NONE,
                )?))],
                else_: Vec::new(),
            })
        };
        for (right, ancestors) in firsts {
            let method = reflected(right).expect("a reflected method");
            let left = ancestors
                .into_iter()
                .map(|left| is_exact(0, left))
                .reduce(|a, b| Expr::Logic {
                    op: LogicOp::Or,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .expect("an ancestor on the left");
            let cond = Expr::Logic {
                op: LogicOp::And,
                left: Box::new(is_exact(1, right)),
                right: Box::new(left),
            };
            body.push(arm(cond, 1, right, method)?);
        }
        for class in &lefts {
            if let Some(method) = direct(*class) {
                body.push(arm(is_exact(0, *class), 0, *class, method)?);
            }
        }
        // The method of the left object decides when every object that the
        // left operand may be has one.
        let decided =
            matches!(reps[0], Rep::Object(_)) && lefts.iter().all(|left| direct(*left).is_some());
        if !decided {
            for class in &rights {
                if let Some(method) = reflected(*class) {
                    body.push(arm(is_exact(1, *class), 1, *class, method)?);
                }
            }
        }
        body.extend(rest(operand(0), operand(1))?);
        let func = self.generated(reps.to_vec(), ret, body);
        Ok(generated_call(func, vec![left.0, right.0], ret, site))
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

/// The method that the arithmetic operator `op` calls on the left object
/// and the reflected one that it calls on the right object, and `None`
/// for an operator that the compiler does not call on an object.
fn arith_methods(op: ast::Operator) -> Option<(&'static str, &'static str)> {
    match op {
        ast::Operator::Add => Some(("__add__", "__radd__")),
        ast::Operator::Sub => Some(("__sub__", "__rsub__")),
        ast::Operator::Mult => Some(("__mul__", "__rmul__")),
        ast::Operator::Div => Some(("__truediv__", "__rtruediv__")),
        ast::Operator::FloorDiv => Some(("__floordiv__", "__rfloordiv__")),
        ast::Operator::Mod => Some(("__mod__", "__rmod__")),
        ast::Operator::Pow => Some(("__pow__", "__rpow__")),
        _ => None,
    }
}
