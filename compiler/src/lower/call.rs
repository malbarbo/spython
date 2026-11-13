//! Calls of the functions of the module and of the builtins.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{
    ArgBinding, CallTarget, Shape, call_bindings, call_target, expr_shape,
};

use super::class::{generated_call, param};
use super::expr::float_value;
use super::{Lowering, Sig, params_of, unsupported};
use crate::Error;
use crate::ir::{Binding, CmpOp, Expr, FuncId, MinMaxOp, Place, Rep, Site, Stmt, VarId};

impl<'a> Lowering<'a, '_> {
    /// A call of a function of the module, with positional arguments.
    pub(super) fn call(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if let ast::Expr::Attribute(attr) = &*call.func {
            return self.method_call(call, attr);
        }
        let ast::Expr::Name(name) = &*call.func else {
            return self.call_value(call);
        };
        let callee = name.id.as_str();
        if self.is_variable(callee) {
            return self.call_value(call);
        }
        // The checker sees no function, and binds no argument, in a name
        // that the `def` has yet to bind.
        let target = call_target(self.model(), call);
        if let Some(CallTarget::Constructor(class)) = &target {
            let class = self.classes.borrow().ids.get(class).copied();
            if let Some(class) = class {
                return self.construct(call, name, class);
            }
            // The prelude makes an object of a copy of its own generic
            // class, whose type gives the copy.
            if self.in_prelude()
                && let Some(Rep::Object(class)) =
                    self.rep_of(expr_shape(self.model(), &ast::Expr::Call(call.clone())))
                && self.copy_file(class).is_some()
            {
                let args = call
                    .arguments
                    .args
                    .iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<_, _>>()?;
                return Ok(self.new_copy(class, args, self.site(call.range())));
            }
        }
        // A name with more than one `def` has the function of the last `def`
        // that ran, which the checker knows only in the code of the module.
        let redefined = self.funcs.borrow().redefined.get(callee).cloned();
        if let Some((current, defs)) = redefined {
            let known = match &target {
                Some(CallTarget::Function(def)) if self.scope.borrow().is_none() => {
                    self.funcs.borrow().ids.get(def).copied()
                }
                _ => None,
            };
            if known.is_none() {
                return self.redefined_call(call, name, current, &defs);
            }
        }
        let (func, bindings) = match target {
            Some(CallTarget::Function(def)) if Some(def.file) == self.intrinsics => {
                return self.intrinsic(call, &def.name);
            }
            // Each call of a generic function of the prelude picks the copy
            // for the reps of its arguments.
            Some(CallTarget::Function(def)) if self.is_generic(&def) => {
                return self.generic_call(call, &def);
            }
            Some(CallTarget::Function(def)) => {
                let known = self.funcs.borrow().ids.get(&def).copied();
                let func = match known {
                    Some(func) => Some(func),
                    None => self.prelude_def(&def)?,
                };
                (func, call_bindings(self.model(), call))
            }
            _ => (self.funcs.borrow().names.get(callee).copied(), None),
        };
        let Some(func) = func else {
            // The checker sees each class of a name with two classes in a
            // function, which may run when the name has either one.
            if self.classes.borrow().names.contains_key(callee) {
                return Err(unsupported(call, "a call of a name of two classes"));
            }
            return Err(unsupported(
                call,
                "a call of a function that the module does not define",
            ));
        };
        // The `def` is the one binding of the name in the module, so the call
        // finds the function, or raises `NameError` before the `def` runs.
        let sig = self.funcs.borrow().get(func).clone();
        // The prelude has every function of its own from the start.
        let check = if self.in_prelude() {
            None
        } else {
            self.global_check(name)?
        };
        let (args, order) = self.arguments(call, &sig, bindings.as_deref())?;
        Ok(Expr::Call {
            func,
            args,
            order,
            rep: sig.ret,
            site: self.call_site(func, call),
            check,
        })
    }

    /// `call` of a value of a type of function, with an argument for each
    /// parameter, in order.
    fn call_value(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let callee = self.expr(&call.func)?;
        let Rep::Func(ty) = callee.rep() else {
            return Err(unsupported(&*call.func, "a call of a value of this type"));
        };
        let ty = self.func_type_of(ty);
        if !call.arguments.keywords.is_empty() || call.arguments.args.len() != ty.params.len() {
            return Err(unsupported(
                call,
                "a call of a function value with these arguments",
            ));
        }
        let args = call
            .arguments
            .args
            .iter()
            .zip(&ty.params)
            .map(|(arg, rep)| {
                self.coerce(self.expr(arg)?, *rep)
                    .ok_or_else(|| unsupported(arg, "an argument of this type"))
            })
            .collect::<Result<_, _>>()?;
        let site = self.site(call.range());
        Ok(Expr::CallValue {
            callee: Box::new(callee),
            args,
            rep: ty.ret,
            site,
            limit: site,
        })
    }

    /// `call` of `name`, a name with the functions `defs`, which calls the
    /// one at the place that the global `current` holds. The arguments run
    /// once, and a generated function takes them with the place and calls
    /// the function there, which binds them by its own parameters.
    fn redefined_call(
        &self,
        call: &ast::ExprCall,
        name: &ast::ExprName,
        current: VarId,
        defs: &[FuncId],
    ) -> Result<Expr, Error> {
        let values = call
            .arguments
            .iter_source_order()
            .map(|arg| self.expr(arg.value()))
            .collect::<Result<Vec<_>, Error>>()?;
        let reps: Vec<Rep> = values.iter().map(Expr::rep).collect();
        let value = |i: usize, _: &ast::Expr| {
            let rep = *reps.get(i).expect("an argument of the call");
            Ok(param(i + 1, rep))
        };
        let calls = defs
            .iter()
            .map(|func| {
                let sig = self.funcs.borrow().get(*func).clone();
                let (args, order) = self.arguments_with(call, &sig, None, &value)?;
                Ok(Expr::Call {
                    func: *func,
                    args,
                    order,
                    rep: sig.ret,
                    site: Some(Site::NONE),
                    check: None,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let rep = calls.first().expect("a name with defs").rep();
        if calls.iter().any(|call| call.rep() != rep) {
            return Err(unsupported(
                call,
                "a call of a name whose functions give two types",
            ));
        }
        let mut body: Vec<Stmt> = calls
            .into_iter()
            .enumerate()
            .map(|(place, call)| Stmt::If {
                cond: Expr::Compare(
                    Box::new(param(0, Rep::Int)),
                    vec![(
                        CmpOp::Eq,
                        Expr::Int(i32::try_from(place).expect("few defs")),
                    )],
                ),
                then: vec![Stmt::Return(Some(call))],
                else_: Vec::new(),
            })
            .collect();
        body.push(Stmt::Unreachable);
        let params = std::iter::once(Rep::Int)
            .chain(reps.iter().copied())
            .collect();
        let func = self.generated(params, rep, body);
        // The read of the global raises `NameError` before any `def` of the
        // name runs, and before the arguments, as in CPython.
        let current = Expr::Load {
            place: Place::Global(current),
            rep: Rep::Int,
            check: self.global_check(name)?,
        };
        let args = std::iter::once(current).chain(values).collect();
        Ok(generated_call(func, args, rep, self.site(call.range())))
    }

    /// The arguments of `call` in the order of the call, and the argument
    /// of each parameter of `sig`, or `None` when each parameter takes the
    /// argument in its place. Without the `bindings` of the checker, a
    /// keyword argument goes to the parameter of its name.
    pub(super) fn arguments(
        &self,
        call: &ast::ExprCall,
        sig: &Sig<'a, '_>,
        bindings: Option<&[ArgBinding]>,
    ) -> Result<(Vec<Expr>, Option<Vec<u32>>), Error> {
        self.arguments_with(call, sig, bindings, &|_, arg| self.expr(arg))
    }

    /// [`Self::arguments`] with the value of each argument from `value`,
    /// which takes its place in the call and its node.
    fn arguments_with(
        &self,
        call: &ast::ExprCall,
        sig: &Sig<'a, '_>,
        bindings: Option<&[ArgBinding]>,
        value: &dyn Fn(usize, &ast::Expr) -> Result<Expr, Error>,
    ) -> Result<(Vec<Expr>, Option<Vec<u32>>), Error> {
        let mut order: Vec<Option<u32>> = sig.params.iter().map(|_| None).collect();
        let mut args = Vec::new();
        for (i, arg) in call.arguments.iter_source_order().enumerate() {
            let param = match (bindings, arg) {
                (Some(bindings), _) => bindings.get(i).and_then(|binding| binding.parameter),
                (None, ast::ArgOrKeyword::Arg(_)) => Some(i),
                (None, ast::ArgOrKeyword::Keyword(keyword)) => {
                    keyword.arg.as_ref().and_then(|name| {
                        params_of(sig.def.expect("a call of a function with a def"))
                            .position(|param| param.name().as_str() == name.as_str())
                    })
                }
            };
            let slot = param
                .and_then(|param| Some((order.get_mut(param)?, sig.params.get(param)?)))
                .filter(|(slot, _)| slot.is_none());
            let Some((slot, rep)) = slot else {
                return Err(unsupported(call, "a call with these arguments"));
            };
            let lowered = self
                .coerce(value(i, arg.value())?, *rep)
                .ok_or_else(|| unsupported(arg.value(), "an argument of this type"))?;
            *slot = Some(u32::try_from(args.len()).expect("fewer than 2^32 arguments"));
            args.push(lowered);
        }
        // The `def` gives each default its value, and a call that finds the
        // function comes after the `def`.
        for ((slot, default), rep) in order.iter_mut().zip(&sig.defaults).zip(&sig.params) {
            if let (None, Some(var)) = (&slot, default) {
                *slot = Some(u32::try_from(args.len()).expect("fewer than 2^32 arguments"));
                args.push(Expr::Load {
                    place: Place::Global(*var),
                    rep: *rep,
                    check: None,
                });
            }
        }
        let order: Vec<u32> = order
            .into_iter()
            .collect::<Option<_>>()
            .ok_or_else(|| unsupported(call, "a call with these arguments"))?;
        let in_place = order.iter().map(|arg| *arg as usize).eq(0..args.len());
        Ok((args, (!in_place).then_some(order)))
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
                let value = self.expr(arg)?;
                // The write of an object calls the `str` of its class.
                match value.rep() {
                    // The write of a list calls its `repr`.
                    Rep::List(list) | Rep::ListOrNone(list) => {
                        self.list_repr_func(list)?;
                    }
                    Rep::Object(class) | Rep::ObjectOrNone(class) => {
                        self.class_conversion(class, false);
                    }
                    Rep::Uniform => {
                        for class in self.classes_in(expr_shape(self.model(), arg)) {
                            self.class_conversion(class, false);
                        }
                    }
                    // CPython writes the address of the function too, which
                    // changes from one run to the next.
                    Rep::Func(_) => return Err(unsupported(arg, "print of a function")),
                    _ => {}
                }
                Ok(value)
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print {
            args,
            site: self.site(call.range()),
        })
    }

    /// `len` of one `str` or one list.
    pub(super) fn len(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of len"));
        };
        let operand = self.expr(arg)?;
        if operand.rep().class().is_some() {
            return self.special_len(call, operand);
        }
        if !matches!(operand.rep(), Rep::Str | Rep::List(_)) {
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
            Rep::Str
            | Rep::None
            | Rep::StrOrNone
            | Rep::Uniform
            | Rep::Object(_)
            | Rep::ObjectOrNone(_)
            | Rep::List(_)
            | Rep::ListOrNone(_)
            | Rep::Hash
            | Rep::Func(_) => Err(unsupported(arg, "abs of this type")),
        }
    }

    /// `hash` of a value, as CPython gives it. CPython raises `TypeError`
    /// for a value without a `hash`, and the compiler refuses its type, as
    /// it does for a key of a `dict`.
    pub(super) fn hash(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of hash")?;
        if !expr_shape(self.model(), arg).is_some_and(|shape| self.hashable(&shape)) {
            return Err(unsupported(arg, "hash of this type"));
        }
        let value = self.expr(arg)?;
        let site = self.site(call.range());
        Ok(Expr::HashAsInt {
            value: Box::new(self.hash_value(arg, value, site)?),
            site,
        })
    }

    /// `min` or `max` of two or more values of one type. An `int` and a
    /// `bool` are two types here, because the result keeps its type.
    pub(super) fn min_max(&self, call: &ast::ExprCall, op: MinMaxOp) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return self.min_max_key(call, op);
        }
        let [first, rest @ ..] = &*call.arguments.args else {
            return Err(unsupported(call, "min or max of an iterable"));
        };
        if rest.is_empty() {
            return self.min_max_of(call, first, op);
        }
        let first = self.expr(first)?;
        let rep = first.rep();
        if matches!(
            rep,
            Rep::None | Rep::StrOrNone | Rep::Uniform | Rep::Object(_) | Rep::ObjectOrNone(_)
        ) {
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
        self.binary(
            call,
            (ast::Operator::Pow, false),
            (left, base),
            (right, exp),
        )
    }

    /// `round(x)` or `round(x, None)` of a number, which gives an `int`,
    /// and `round(x, n)`, which keeps the type of `x`. Only the literal
    /// `None` stands for the missing number of digits.
    pub(super) fn round(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let (arg, digits) = match (&*call.arguments.args, &*call.arguments.keywords) {
            ([arg] | [arg, ast::Expr::NoneLiteral(_)], []) => (arg, None),
            ([arg, digits], []) => (arg, Some(digits)),
            _ => return Err(unsupported(call, "this call of round")),
        };
        let operand = self.expr(arg)?;
        let site = self.site(call.range());
        match (operand.rep(), digits) {
            (Rep::Int | Rep::Bool, None) => self.int_value(operand, arg),
            (Rep::Int | Rep::Bool, Some(digits)) => Ok(Expr::IntRound {
                operand: Box::new(self.int_value(operand, arg)?),
                digits: Box::new(self.digits(digits)?),
                site,
            }),
            (Rep::Float, Some(digits)) => Ok(Expr::FloatRoundDigits {
                operand: Box::new(operand),
                digits: Box::new(self.digits(digits)?),
                site,
            }),
            (Rep::Float, None) => Ok(Expr::FloatAsInt {
                operand: Box::new(Expr::FloatRound(Box::new(operand))),
                site,
            }),
            (
                Rep::Str
                | Rep::None
                | Rep::StrOrNone
                | Rep::Uniform
                | Rep::Object(_)
                | Rep::ObjectOrNone(_)
                | Rep::List(_)
                | Rep::ListOrNone(_)
                | Rep::Hash
                | Rep::Func(_),
                _,
            ) => Err(unsupported(arg, "round of this type")),
        }
    }

    /// The number of digits of `round`, an `int` or a `bool`.
    fn digits(&self, digits: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(digits)?;
        match lowered.rep() {
            Rep::Int | Rep::Bool => self.int_value(lowered, digits),
            Rep::Float
            | Rep::Str
            | Rep::None
            | Rep::StrOrNone
            | Rep::Uniform
            | Rep::Object(_)
            | Rep::ObjectOrNone(_)
            | Rep::List(_)
            | Rep::ListOrNone(_)
            | Rep::Hash
            | Rep::Func(_) => Err(unsupported(digits, "round to this number of digits")),
        }
    }

    /// `bool()`, which is `False`, or `bool(x)`, which is the truth of `x`.
    pub(super) fn bool(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Bool(false)),
            ([arg], []) => self.value_truth(arg, call),
            _ => Err(unsupported(call, "this call of bool")),
        }
    }

    /// `isinstance(x, t)` of a class of the program, or of `int`, `float`,
    /// `bool`, `str`, `list` or `object`.
    pub(super) fn isinstance(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([value, class], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of isinstance"));
        };
        let lowered = self.expr(value)?;
        let from = lowered.rep();
        let Some(Shape::Class(id)) = expr_shape(self.model(), class) else {
            return Err(unsupported(class, "isinstance with this class"));
        };
        let (rep, check) = if is_builtins(self.model().db(), id.file) {
            let rep = match id.name.as_str() {
                "object" => Some(from),
                // A `bool` is an `int`, which a value narrowed to `int`
                // would no longer write as `True`.
                "int" if self.is_object_shape(value) => {
                    return Err(unsupported(call, "isinstance of an object with int"));
                }
                "int" => instance_of(from, Rep::Int, &[Rep::Bool]),
                "float" => instance_of(from, Rep::Float, &[]),
                "bool" => instance_of(from, Rep::Bool, &[]),
                "str" => instance_of(from, Rep::Str, &[]),
                "list" => match from {
                    Rep::List(_) => Some(from),
                    Rep::ListOrNone(list) => Some(Rep::List(list)),
                    _ => None,
                },
                _ => return Err(unsupported(class, "isinstance with this class")),
            };
            (rep, None)
        } else {
            let ast::Expr::Name(name) = class else {
                return Err(unsupported(class, "isinstance with this class"));
            };
            let Some(target) = self.classes.borrow().ids.get(&id).copied() else {
                return Err(unsupported(class, "isinstance with this class"));
            };
            let rep = match from {
                Rep::Object(class) if self.descends(class, target) => Some(from),
                Rep::ObjectOrNone(class) if self.descends(class, target) => {
                    Some(Rep::Object(class))
                }
                Rep::Object(class) | Rep::ObjectOrNone(class) if self.descends(target, class) => {
                    Some(Rep::Object(target))
                }
                Rep::Uniform => Some(Rep::Object(target)),
                _ => None,
            };
            let check = self
                .global_check(name)?
                .map(|check| (Binding::Class(target), check));
            (rep, check)
        };
        Ok(Expr::IsInstance {
            value: Box::new(lowered),
            rep,
            check,
        })
    }

    /// Returns `true` if the checker gives `node` the type `object`, `false`
    /// otherwise.
    fn is_object_shape(&self, node: &ast::Expr) -> bool {
        matches!(expr_shape(self.model(), node), Some(Shape::Instance(class, _)) if self.is_object(&class))
    }

    /// `float()`, which is `0.0`, or `float(x)` of a number.
    pub(super) fn float(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Float(0.0)),
            ([arg], []) => {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::Int | Rep::Bool | Rep::Float => float_value(lowered, arg),
                    Rep::Str => self.prelude_call(call, "float", "float_of_str", vec![lowered]),
                    Rep::None
                    | Rep::StrOrNone
                    | Rep::Uniform
                    | Rep::Object(_)
                    | Rep::ObjectOrNone(_)
                    | Rep::List(_)
                    | Rep::ListOrNone(_)
                    | Rep::Hash
                    | Rep::Func(_) => Err(unsupported(arg, "float of this type")),
                }
            }
            _ => Err(unsupported(call, "this call of float")),
        }
    }

    /// `str()`, which is `''`, or `str(x)` of a number or of a `str`.
    pub(super) fn str(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Str(String::new())),
            ([arg], []) => self.str_of(self.expr(arg)?, arg, call.range()),
            _ => Err(unsupported(call, "this call of str")),
        }
    }

    /// `repr(x)` of a value of any rep.
    pub(super) fn repr(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of repr")?;
        let lowered = self.expr(arg)?;
        self.repr_of(lowered, call.range())
    }

    /// The `repr` of `lowered`. The conversion covers `range`.
    pub(super) fn repr_of(&self, lowered: Expr, range: TextRange) -> Result<Expr, Error> {
        let site = self.site(range);
        self.repr_at(lowered, site, range)
    }

    /// The `repr` of `value` at `site`, for the code at `range`.
    pub(super) fn repr_at(&self, value: Expr, site: Site, range: TextRange) -> Result<Expr, Error> {
        Ok(match value.rep() {
            Rep::Int | Rep::Float | Rep::Bool => Expr::StrOf {
                operand: Box::new(value),
                site,
            },
            Rep::Str => self.prelude_call_at("str", "repr_of", vec![value], site)?,
            // An object may be null.
            Rep::Object(class) | Rep::ObjectOrNone(class) => generated_call(
                self.or_none_conversion(class, true)?,
                vec![value],
                Rep::Str,
                site,
            ),
            Rep::None | Rep::StrOrNone => {
                let value = self.coerce(value, Rep::StrOrNone).expect("a str or None");
                self.prelude_call_at("str", "repr_or_none", vec![value], site)?
            }
            Rep::List(_) | Rep::ListOrNone(_) => self.list_repr(value, site)?,
            // The `repr` of a uniform value tests its type when it runs.
            Rep::Uniform => {
                self.uniform_repr.set(true);
                Expr::UniformRepr {
                    value: Box::new(self.coerce(value, Rep::Uniform).expect("a uniform value")),
                    site,
                }
            }
            Rep::Hash => unreachable!("the prelude writes no hash"),
            // CPython writes the address of the function too, which changes
            // from one run to the next.
            Rep::Func(_) => {
                return Err(unsupported(&range, "the repr of a function"));
            }
        })
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
                    Rep::Str => {
                        self.prelude_call(call, "int", "int_of_str", vec![lowered, Expr::Int(10)])
                    }
                    Rep::None
                    | Rep::StrOrNone
                    | Rep::Uniform
                    | Rep::Object(_)
                    | Rep::ObjectOrNone(_)
                    | Rep::List(_)
                    | Rep::ListOrNone(_)
                    | Rep::Hash
                    | Rep::Func(_) => Err(unsupported(arg, "int of this type")),
                }
            }
            ([arg, base], []) => self.int_with_base(call, arg, base),
            ([arg], [keyword])
                if keyword
                    .arg
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "base") =>
            {
                self.int_with_base(call, arg, &keyword.value)
            }
            _ => Err(unsupported(call, "this call of int")),
        }
    }

    /// `int(arg, base)` of a `str`, which the prelude defines.
    fn int_with_base(
        &self,
        call: &ast::ExprCall,
        arg: &ast::Expr,
        base: &ast::Expr,
    ) -> Result<Expr, Error> {
        let lowered = self.expr(arg)?;
        if lowered.rep() != Rep::Str {
            return Err(unsupported(arg, "int with a base of this type"));
        }
        let base = self.int_operand(base)?;
        self.prelude_call(call, "int", "int_of_str", vec![lowered, base])
    }

    /// `ord` of a `str`, which the prelude defines.
    pub(super) fn ord(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of ord")?;
        let lowered = self.expr(arg)?;
        if lowered.rep() != Rep::Str {
            return Err(unsupported(arg, "ord of this type"));
        }
        self.prelude_call(call, "str", "ord_of", vec![lowered])
    }

    /// `chr` of an `int`, which the prelude defines.
    pub(super) fn chr(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of chr")?;
        let lowered = self.int_operand(arg)?;
        self.prelude_call(call, "str", "chr_of", vec![lowered])
    }

    /// The name of the function or the class of the builtins that `func`
    /// reads. `None` when `func` reads neither.
    pub(super) fn builtin_name(&self, func: &ast::Expr) -> Option<String> {
        let (name, file) = match expr_shape(self.model(), func)? {
            Shape::Function(def) => (def.name, def.file),
            Shape::Class(class) => (class.name, class.file),
            _ => return None,
        };
        is_builtins(self.model().db(), file).then_some(name)
    }
}

/// The rep that [`Expr::IsInstance`] tests for a value of `from` and the
/// builtin type of the values of `rep`, whose values are also the values
/// of `others`.
fn instance_of(from: Rep, rep: Rep, others: &[Rep]) -> Option<Rep> {
    match from {
        _ if from == rep || others.contains(&from) => Some(from),
        Rep::Uniform => Some(rep),
        Rep::StrOrNone if rep == Rep::Str => Some(rep),
        _ => None,
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
pub(super) fn is_builtins(db: &dyn Db, file: File) -> bool {
    is_module(db, file, KnownModule::Builtins)
}

/// Returns `true` if `file` is the module `known`, `false` otherwise.
pub(super) fn is_module(db: &dyn Db, file: File, known: KnownModule) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, known))
}
