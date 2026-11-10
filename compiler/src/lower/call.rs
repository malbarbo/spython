//! Calls of the functions of the module and of the builtins.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{
    ArgBinding, CallTarget, Shape, call_bindings, call_target, expr_shape,
};

use super::expr::float_value;
use super::{Lowering, Sig, coerce, params_of, unsupported};
use crate::Error;
use crate::ir::{Expr, MinMaxOp, Place, Rep};

impl<'a> Lowering<'a, '_> {
    /// A call of a function of the module, with positional arguments.
    pub(super) fn call(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if let ast::Expr::Attribute(attr) = &*call.func {
            return self.method_call(call, attr);
        }
        let ast::Expr::Name(name) = &*call.func else {
            return Err(unsupported(
                call,
                "a call of an expression other than a name",
            ));
        };
        let callee = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(callee))
        {
            return Err(unsupported(call, "a call of a local variable"));
        }
        // The checker sees no function, and binds no argument, in a name
        // that the `def` has yet to bind.
        let (func, bindings) = match call_target(self.model(), call) {
            Some(CallTarget::Function(def)) if Some(def.file) == self.intrinsics => {
                return self.intrinsic(call, &def.name);
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

    /// The arguments of `call` in the order of the call, and the argument
    /// of each parameter of `sig`. Without the `bindings` of the checker, a
    /// keyword argument goes to the parameter of its name.
    fn arguments(
        &self,
        call: &ast::ExprCall,
        sig: &Sig<'a, '_>,
        bindings: Option<&[ArgBinding]>,
    ) -> Result<(Vec<Expr>, Vec<u32>), Error> {
        let mut order: Vec<Option<u32>> = sig.params.iter().map(|_| None).collect();
        let mut args = Vec::new();
        for (i, arg) in call.arguments.iter_source_order().enumerate() {
            let param = match (bindings, arg) {
                (Some(bindings), _) => bindings.get(i).and_then(|binding| binding.parameter),
                (None, ast::ArgOrKeyword::Arg(_)) => Some(i),
                (None, ast::ArgOrKeyword::Keyword(keyword)) => {
                    keyword.arg.as_ref().and_then(|name| {
                        params_of(sig.def).position(|param| param.name().as_str() == name.as_str())
                    })
                }
            };
            let slot = param
                .and_then(|param| Some((order.get_mut(param)?, sig.params.get(param)?)))
                .filter(|(slot, _)| slot.is_none());
            let Some((slot, rep)) = slot else {
                return Err(unsupported(call, "a call with these arguments"));
            };
            let lowered = coerce(self.expr(arg.value())?, *rep)
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
        let order = order
            .into_iter()
            .collect::<Option<_>>()
            .ok_or_else(|| unsupported(call, "a call with these arguments"))?;
        Ok((args, order))
    }

    pub(super) fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| self.expr(arg))
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print {
            args,
            site: self.site(call.range()),
        })
    }

    /// `len` of one `str`.
    pub(super) fn len(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of len"));
        };
        let operand = self.expr(arg)?;
        if operand.rep() != Rep::Str {
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
            Rep::Str | Rep::None | Rep::StrOrNone | Rep::Uniform => {
                Err(unsupported(arg, "abs of this type"))
            }
        }
    }

    /// `min` or `max` of two or more values of one type. An `int` and a
    /// `bool` are two types here, because the result keeps its type.
    pub(super) fn min_max(&self, call: &ast::ExprCall, op: MinMaxOp) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "min or max with keyword arguments"));
        }
        let [first, rest @ ..] = &*call.arguments.args else {
            return Err(unsupported(call, "min or max of an iterable"));
        };
        if rest.is_empty() {
            return Err(unsupported(call, "min or max of an iterable"));
        }
        let first = self.expr(first)?;
        let rep = first.rep();
        if matches!(rep, Rep::None | Rep::StrOrNone | Rep::Uniform) {
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
        self.binary(call, ast::Operator::Pow, (left, base), (right, exp))
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
            (Rep::Str | Rep::None | Rep::StrOrNone | Rep::Uniform, _) => {
                Err(unsupported(arg, "round of this type"))
            }
        }
    }

    /// The number of digits of `round`, an `int` or a `bool`.
    fn digits(&self, digits: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(digits)?;
        match lowered.rep() {
            Rep::Int | Rep::Bool => self.int_value(lowered, digits),
            Rep::Float | Rep::Str | Rep::None | Rep::StrOrNone | Rep::Uniform => {
                Err(unsupported(digits, "round to this number of digits"))
            }
        }
    }

    /// `bool()`, which is `False`, or `bool(x)`, which is the truth of `x`.
    pub(super) fn bool(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        match (&*call.arguments.args, &*call.arguments.keywords) {
            ([], []) => Ok(Expr::Bool(false)),
            ([arg], []) => self.truth_of(arg),
            _ => Err(unsupported(call, "this call of bool")),
        }
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
                    Rep::None | Rep::StrOrNone | Rep::Uniform => {
                        Err(unsupported(arg, "float of this type"))
                    }
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

    /// `repr(x)` of a number or of a `str`.
    pub(super) fn repr(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let arg = one_arg(call, "this call of repr")?;
        let lowered = self.expr(arg)?;
        self.repr_of(lowered, arg, call.range())
    }

    /// The `repr` of `lowered`, the value of `node`. The conversion covers
    /// `range`.
    pub(super) fn repr_of(
        &self,
        lowered: Expr,
        node: &impl Ranged,
        range: TextRange,
    ) -> Result<Expr, Error> {
        match lowered.rep() {
            Rep::Int | Rep::Float | Rep::Bool => self.str_of(lowered, node, range),
            Rep::Str => self.prelude_call(&range, "str", "repr_of", vec![lowered]),
            Rep::None | Rep::StrOrNone | Rep::Uniform => {
                Err(unsupported(node, "repr of this type"))
            }
        }
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
                    Rep::None | Rep::StrOrNone | Rep::Uniform => {
                        Err(unsupported(arg, "int of this type"))
                    }
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
        let lowered = self.expr(arg)?;
        let lowered = match lowered.rep() {
            Rep::Int => lowered,
            Rep::Bool => Expr::BoolAsInt(Box::new(lowered)),
            _ => return Err(unsupported(arg, "chr of this type")),
        };
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

/// The one positional argument of `call`, which has no keyword argument.
fn one_arg<'c>(call: &'c ast::ExprCall, what: &str) -> Result<&'c ast::Expr, Error> {
    match (&*call.arguments.args, &*call.arguments.keywords) {
        ([arg], []) => Ok(arg),
        _ => Err(unsupported(call, what)),
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
pub(super) fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}
