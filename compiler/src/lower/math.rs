//! The names of `math`, which an import binds.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::expr::float_value;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, FloatClass, MathFunc, Rep};

impl<'a> Lowering<'a, '_> {
    /// The name of the import in `expr` and the name in `math` that `expr`
    /// reads, as `math.pi` reads `pi` through the module and `pi` reads it
    /// after `from math import pi`. `None` when `expr` reads no name of
    /// `math`.
    pub(super) fn math_member<'e>(
        &self,
        expr: &'e ast::Expr,
    ) -> Option<(&'e ast::ExprName, String)> {
        let (imported, attr) = match expr {
            ast::Expr::Name(imported) => (imported, None),
            ast::Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => match &**value {
                ast::Expr::Name(imported) => (imported, Some(attr.as_str())),
                _ => return None,
            },
            _ => return None,
        };
        let name = imported.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(name))
        {
            return None;
        }
        let member = self.imports.borrow().members.get(name)?.clone();
        match (member, attr) {
            (Some(member), None) => Some((imported, member)),
            (None, Some(attr)) => Some((imported, attr.to_owned())),
            _ => None,
        }
    }

    /// `value`, which reads a name of `math` through `imported`. A read
    /// before the import raises `NameError`.
    fn after_import(&self, imported: &ast::ExprName, value: Expr) -> Result<Expr, Error> {
        let import = *self
            .imports
            .borrow()
            .ids
            .get(imported.id.as_str())
            .expect("a declared import");
        Ok(match self.global_check(imported)? {
            Some(check) => Expr::AfterImport {
                import,
                check,
                value: Box::new(value),
            },
            None => value,
        })
    }

    /// The value of `member`, a constant of `math`, through `imported`.
    pub(super) fn math_constant(
        &self,
        expr: &ast::Expr,
        imported: &ast::ExprName,
        member: &str,
    ) -> Result<Expr, Error> {
        let value = match member {
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "tau" => std::f64::consts::TAU,
            "inf" => f64::INFINITY,
            "nan" => f64::NAN,
            _ => return Err(unsupported(expr, "this name of math")),
        };
        self.after_import(imported, Expr::Float(value))
    }

    /// A call of `member`, a function of `math`, through `imported`.
    /// `math.floor`, `math.ceil` and `math.trunc` give an `int`,
    /// `math.isnan`, `math.isinf` and `math.isfinite` give a `bool`, and
    /// the others give a `float`.
    pub(super) fn math_call(
        &self,
        call: &ast::ExprCall,
        imported: &ast::ExprName,
        member: &str,
    ) -> Result<Expr, Error> {
        let known = matches!(
            member,
            "floor"
                | "ceil"
                | "trunc"
                | "sqrt"
                | "isnan"
                | "isinf"
                | "isfinite"
                | "fabs"
                | "copysign"
                | "fmod"
                | "gcd"
                | "atan2"
                | "hypot"
                | "log"
        ) || MathFunc::of(member).is_some();
        if !known {
            return Err(unsupported(call, "this function of math"));
        }
        let value = match (member, &*call.arguments.args, &*call.arguments.keywords) {
            ("gcd", args, []) => Expr::IntGcd {
                args: args
                    .iter()
                    .map(|arg| self.math_int(arg))
                    .collect::<Result<_, _>>()?,
                site: self.site(call.range()),
            },
            ("atan2", [y, x], []) => Expr::Atan2 {
                y: Box::new(self.math_float(y)?),
                x: Box::new(self.math_float(x)?),
            },
            ("hypot", [x, y], []) => Expr::Hypot {
                x: Box::new(self.math_float(x)?),
                y: Box::new(self.math_float(y)?),
            },
            ("copysign", [x, y], []) => Expr::Copysign {
                x: Box::new(self.math_float(x)?),
                y: Box::new(self.math_float(y)?),
            },
            ("fmod", [x, y], []) => Expr::Fmod {
                x: Box::new(self.math_float(x)?),
                y: Box::new(self.math_float(y)?),
                site: self.site(call.range()),
            },
            ("log", [x, base], []) => Expr::LogBase {
                operand: Box::new(self.math_float(x)?),
                base: Box::new(self.math_float(base)?),
                site: self.site(call.range()),
            },
            (_, [arg], []) if !matches!(member, "atan2" | "hypot" | "copysign" | "fmod") => {
                self.math_unary(call, member, arg)?
            }
            _ => return Err(unsupported(call, "this call of a function of math")),
        };
        self.after_import(imported, value)
    }

    /// A call of `member`, a function of `math` of one number.
    fn math_unary(
        &self,
        call: &ast::ExprCall,
        member: &str,
        arg: &ast::Expr,
    ) -> Result<Expr, Error> {
        let class = match member {
            "isnan" => Some(FloatClass::Nan),
            "isinf" => Some(FloatClass::Inf),
            "isfinite" => Some(FloatClass::Finite),
            _ => None,
        };
        if let Some(class) = class {
            return Ok(Expr::FloatIs {
                class,
                operand: Box::new(self.math_float(arg)?),
            });
        }
        if let Some(func) = MathFunc::of(member) {
            return Ok(Expr::MathCall {
                func,
                operand: Box::new(self.math_float(arg)?),
                site: self.site(call.range()),
            });
        }
        if member == "fabs" {
            return Ok(Expr::FloatAbs(Box::new(self.math_float(arg)?)));
        }
        if member == "sqrt" {
            return Ok(Expr::FloatSqrt {
                operand: Box::new(self.math_float(arg)?),
                site: self.site(call.range()),
            });
        }
        let operand = self.math_number(arg)?;
        if operand.rep() != Rep::Float {
            return self.int_value(operand, arg);
        }
        let operand = Box::new(operand);
        let whole = match member {
            "floor" => Expr::FloatFloor(operand),
            "ceil" => Expr::FloatCeil(operand),
            // `int` of a `float` drops its fraction.
            _ => *operand,
        };
        Ok(Expr::FloatAsInt {
            operand: Box::new(whole),
            site: self.site(call.range()),
        })
    }

    /// The number of `arg`, an `int`, a `bool` or a `float`.
    fn math_number(&self, arg: &ast::Expr) -> Result<Expr, Error> {
        let operand = self.expr(arg)?;
        match operand.rep() {
            Rep::Int | Rep::Bool | Rep::Float => Ok(operand),
            Rep::Str | Rep::None => Err(unsupported(arg, "a function of math of this type")),
        }
    }

    /// The `int` of `arg`, an `int` or a `bool`.
    fn math_int(&self, arg: &ast::Expr) -> Result<Expr, Error> {
        let operand = self.expr(arg)?;
        match operand.rep() {
            Rep::Int => Ok(operand),
            Rep::Bool => Ok(Expr::BoolAsInt(Box::new(operand))),
            Rep::Float | Rep::Str | Rep::None => {
                Err(unsupported(arg, "a function of math of this type"))
            }
        }
    }

    /// The number of `arg` as a `float`.
    fn math_float(&self, arg: &ast::Expr) -> Result<Expr, Error> {
        float_value(self.math_number(arg)?, arg)
    }
}
