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

    /// `left op= right` of an augmented assignment `node`, the value that
    /// goes to the target. An object on the left whose class has the
    /// method of the assignment, as `__iadd__` for `+=`, calls it, and the
    /// other values take the value of `left op right`.
    pub(super) fn augmented(
        &self,
        node: &impl Ranged,
        op: ast::Operator,
        (left, left_node): (Expr, &impl Ranged),
        (right, right_node): (Expr, &ast::Expr),
    ) -> Result<Expr, Error> {
        let method = |class, name| self.method(class, name).and_then(|method| method.func);
        let name = inplace_method(op);
        let lefts = self.classes_of(left.rep(), || None);
        let inplace = |class| name.and_then(|name| method(class, name));
        if !lefts.iter().any(|class| inplace(*class).is_some()) {
            return self.binary(node, op, (left, left_node), (right, right_node));
        }
        let Rep::Object(class) = left.rep() else {
            return Err(unsupported(node, "an operator on an object of a union"));
        };
        let site = self.site(node.range());
        if let Some(func) = inplace(class)
            && lefts.iter().all(|left| inplace(*left) == Some(func))
        {
            return self.call_operator(node, func, left, right, site);
        }
        let reps = [left.rep(), right.rep()];
        let operand = |i| param(i, if i == 0 { reps[0] } else { reps[1] });
        let mut arms = Vec::new();
        for class in &lefts {
            if let Some(func) = inplace(*class) {
                let exact = Expr::AsExact {
                    value: Box::new(operand(0)),
                    class: *class,
                };
                let call = self.call_operator(node, func, exact, operand(1), Site::NONE)?;
                arms.push((*class, call));
            }
        }
        let rest = if lefts.iter().all(|left| inplace(*left).is_some()) {
            None
        } else {
            Some(self.special_arith(node, op, operand(0), operand(1))?)
        };
        let ret = arms
            .iter()
            .map(|(_, call)| call.rep())
            .chain(rest.as_ref().map(Expr::rep))
            .reduce(|a, b| self.join(a, b))
            .expect("a method of the assignment");
        let coerce = |value| {
            self.coerce(value, ret)
                .expect("the join holds every result")
        };
        let mut body: Vec<Stmt> = arms
            .into_iter()
            .map(|(class, call)| Stmt::If {
                cond: Expr::IsExact {
                    value: Box::new(operand(0)),
                    class,
                },
                then: vec![Stmt::Return(Some(coerce(call)))],
                else_: Vec::new(),
            })
            .collect();
        body.push(match rest {
            Some(value) => Stmt::Return(Some(coerce(value))),
            None => Stmt::Unreachable,
        });
        let func = self.generated(reps.to_vec(), ret, body);
        Ok(generated_call(func, vec![left, right], ret, site))
    }

    /// `len(operand)` of `call`, an object, which calls `__len__` of its
    /// class. A negative result raises `ValueError`.
    pub(super) fn special_len(&self, call: &ast::ExprCall, operand: Expr) -> Result<Expr, Error> {
        let Rep::Object(class) = operand.rep() else {
            return Err(unsupported(call, "len of an object of a union"));
        };
        let method = self
            .method(class, "__len__")
            .ok_or_else(|| unsupported(call, "len of this type"))?;
        let func = self.method_slot(class, method);
        let len = generated_call(func, vec![operand], Rep::Int, self.site(call.range()));
        self.prelude_call(call, "int", "length", vec![len])
    }

    /// The truth of `value`, an object, an object or `None` or a value of
    /// a union, at `node`, as a `bool`, and `None` when no class of the
    /// value has `__bool__` or `__len__`. An object calls the `__bool__` of its class, or else
    /// its `__len__`, which gives `False` for 0. `None` is false.
    pub(super) fn special_truth(
        &self,
        node: &impl Ranged,
        value: Expr,
    ) -> Result<Result<Expr, Expr>, Error> {
        if value.rep() == Rep::Uniform {
            return self.uniform_truth(node, value);
        }
        let Some(class) = value.rep().class() else {
            return Ok(Err(value));
        };
        let classes = self.descendants(class);
        let truths: Vec<Option<(&str, FuncId)>> = classes
            .iter()
            .map(|class| self.truth_method(*class))
            .collect();
        if truths.iter().all(Option::is_none) {
            return Ok(Err(value));
        }
        let site = self.site(node.range());
        let truth = |truth, object| self.object_truth(node, truth, object, site);
        let first = truths.first().copied().flatten();
        let same = truths.iter().all(|truth| *truth == first);
        if same && let Rep::Object(_) = value.rep() {
            return truth(first, value).map(Ok);
        }
        let object = || param(0, Rep::Object(class));
        let mut body = Vec::new();
        if let Rep::ObjectOrNone(_) = value.rep() {
            body.push(Stmt::If {
                cond: Expr::IsNone(Box::new(param(0, value.rep()))),
                then: vec![Stmt::Return(Some(Expr::Bool(false)))],
                else_: Vec::new(),
            });
        }
        let narrowed = || match value.rep() {
            Rep::ObjectOrNone(_) => Expr::Narrow {
                value: Box::new(param(0, value.rep())),
                rep: Rep::Object(class),
            },
            _ => object(),
        };
        if same {
            body.push(Stmt::Return(Some(truth(first, narrowed())?)));
        } else {
            for (exact, own) in classes.iter().zip(&truths) {
                let exact_value = Expr::AsExact {
                    value: Box::new(narrowed()),
                    class: *exact,
                };
                body.push(Stmt::If {
                    cond: Expr::IsExact {
                        value: Box::new(narrowed()),
                        class: *exact,
                    },
                    then: vec![Stmt::Return(Some(truth(*own, exact_value)?))],
                    else_: Vec::new(),
                });
            }
            body.push(Stmt::Unreachable);
        }
        let func = self.generated(vec![value.rep()], Rep::Bool, body);
        Ok(Ok(generated_call(func, vec![value], Rep::Bool, site)))
    }

    /// The truth of `value`, a value of a union, at `node`, and `None` when
    /// no class of the program has `__bool__` or `__len__`. An object of a
    /// class with one of them calls it, and the rest of the values take
    /// the truth of their type.
    fn uniform_truth(&self, node: &impl Ranged, value: Expr) -> Result<Result<Expr, Expr>, Error> {
        let methods: Vec<_> = self
            .all_classes()
            .filter_map(|class| Some((class, self.truth_method(class)?)))
            .collect();
        if methods.is_empty() {
            return Ok(Err(value));
        }
        let site = self.site(node.range());
        let uniform = || param(0, Rep::Uniform);
        let mut body = Vec::new();
        for (class, method) in methods {
            let object = Expr::AsExact {
                value: Box::new(uniform()),
                class,
            };
            body.push(Stmt::If {
                cond: Expr::IsExact {
                    value: Box::new(uniform()),
                    class,
                },
                then: vec![Stmt::Return(Some(self.object_truth(
                    node,
                    Some(method),
                    object,
                    site,
                )?))],
                else_: Vec::new(),
            });
        }
        body.push(Stmt::Return(Some(Expr::Truth(Box::new(uniform())))));
        let func = self.generated(vec![Rep::Uniform], Rep::Bool, body);
        Ok(Ok(generated_call(func, vec![value], Rep::Bool, site)))
    }

    /// The method that gives the truth of an object of `class`, `__bool__`
    /// or else `__len__`, with its name, and `None` when the class has
    /// neither.
    fn truth_method(&self, class: ClassId) -> Option<(&'static str, FuncId)> {
        ["__bool__", "__len__"].into_iter().find_map(|name| {
            let func = self.method(class, name)?.func?;
            Some((name, func))
        })
    }

    /// The truth of `object` at `site`, which calls `method`, the one of
    /// [`Self::truth_method`]. An object without that method is true.
    fn object_truth(
        &self,
        node: &impl Ranged,
        method: Option<(&str, FuncId)>,
        object: Expr,
        site: Site,
    ) -> Result<Expr, Error> {
        let Some((name, func)) = method else {
            return Ok(Expr::Bool(true));
        };
        let sig = self.funcs.borrow().get(func).clone();
        let receiver = *sig.params.first().expect("a method has self");
        let call = generated_call(func, vec![widen(object, receiver)], sig.ret, site);
        if name == "__bool__" {
            return Ok(call);
        }
        let len = self.prelude_call(node, "int", "length", vec![call])?;
        Ok(Expr::Compare(
            Box::new(len),
            vec![(CmpOp::Ne, Expr::Int(0))],
        ))
    }

    /// The call at `node` of the method `name` of the class of an object,
    /// with `args` in the order that they run. `params` has the parameter
    /// of each argument, and the object goes to the parameter 0, `self`.
    fn call_special(
        &self,
        node: &impl Ranged,
        name: &str,
        args: Vec<Expr>,
        params: &[usize],
    ) -> Result<Expr, Error> {
        let object = params
            .iter()
            .position(|param| *param == 0)
            .and_then(|at| args.get(at))
            .expect("the object among the arguments");
        let Rep::Object(class) = object.rep() else {
            return Err(unsupported(node, "an operation on an object of a union"));
        };
        let method = self
            .method(class, name)
            .ok_or_else(|| unsupported(node, "this operation on an object"))?;
        let func = self.method_slot(class, method);
        let sig = self.funcs.borrow().get(func).clone();
        let args = args
            .into_iter()
            .zip(params)
            .map(|(arg, param)| {
                let rep = *sig
                    .params
                    .get(*param)
                    .expect("a parameter of each argument");
                self.coerce(arg, rep)
                    .ok_or_else(|| unsupported(node, "an argument of this type"))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut order = vec![0; params.len()];
        for (arg, param) in params.iter().enumerate() {
            *order
                .get_mut(*param)
                .expect("an argument of each parameter") =
                u32::try_from(arg).expect("fewer than 2^32 arguments");
        }
        let in_order = params.iter().enumerate().all(|(arg, param)| arg == *param);
        Ok(Expr::Call {
            func,
            args,
            order: (!in_order).then_some(order),
            rep: sig.ret,
            site: Some(self.site(node.range())),
            check: None,
        })
    }

    /// `x in container` of `c`, where `container` is an object, which calls
    /// `__contains__` of its class. `x` runs first.
    pub(super) fn special_contains(
        &self,
        c: &ast::ExprCompare,
        x: Expr,
        container: Expr,
    ) -> Result<Expr, Error> {
        self.call_special(c, "__contains__", vec![x, container], &[1, 0])
    }

    /// `object[index]` of `sub`, which calls `__getitem__` of the class of
    /// the object.
    pub(super) fn special_getitem(
        &self,
        sub: &ast::ExprSubscript,
        object: Expr,
        index: Expr,
    ) -> Result<Expr, Error> {
        self.call_special(sub, "__getitem__", vec![object, index], &[0, 1])
    }

    /// `object[index] = value` of `target`, which calls `__setitem__` of the
    /// class of the object. The value runs first, then the object and then
    /// the index, as in CPython.
    pub(super) fn special_setitem(
        &self,
        target: &ast::ExprSubscript,
        value: Expr,
        object: Expr,
        index: Expr,
    ) -> Result<Expr, Error> {
        self.call_special(
            target,
            "__setitem__",
            vec![value, object, index],
            &[2, 0, 1],
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
                .reduce(|a, b| Expr::logic(LogicOp::Or, a, b))
                .expect("an ancestor on the left");
            let cond = Expr::logic(LogicOp::And, is_exact(1, right), left);
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

/// The method of the augmented assignment of `op`, as `__iadd__` for `+=`,
/// and `None` for an operator that the compiler does not call on an
/// object.
fn inplace_method(op: ast::Operator) -> Option<&'static str> {
    match op {
        ast::Operator::Add => Some("__iadd__"),
        ast::Operator::Sub => Some("__isub__"),
        ast::Operator::Mult => Some("__imul__"),
        ast::Operator::Div => Some("__itruediv__"),
        ast::Operator::FloorDiv => Some("__ifloordiv__"),
        ast::Operator::Mod => Some("__imod__"),
        ast::Operator::Pow => Some("__ipow__"),
        _ => None,
    }
}
