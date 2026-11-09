//! The names of `math`, which an import binds.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::expr::float_value;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, FloatClass, Rep};

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

    /// A call of `member`, a function of `math` of one number, through
    /// `imported`. `math.floor`, `math.ceil` and `math.trunc` give an
    /// `int`, `math.sqrt` gives a `float`, and `math.isnan`, `math.isinf`
    /// and `math.isfinite` give a `bool`.
    pub(super) fn math_call(
        &self,
        call: &ast::ExprCall,
        imported: &ast::ExprName,
        member: &str,
    ) -> Result<Expr, Error> {
        let class = match member {
            "isnan" => Some(FloatClass::Nan),
            "isinf" => Some(FloatClass::Inf),
            "isfinite" => Some(FloatClass::Finite),
            _ => None,
        };
        if class.is_none() && !matches!(member, "floor" | "ceil" | "trunc" | "sqrt") {
            return Err(unsupported(call, "this function of math"));
        }
        let arg = match (&*call.arguments.args, &*call.arguments.keywords) {
            ([arg], []) => arg,
            _ => return Err(unsupported(call, "this call of a function of math")),
        };
        let operand = self.expr(arg)?;
        let site = self.site(call.range());
        let value = match (member, operand.rep()) {
            (_, Rep::Str | Rep::None) => {
                return Err(unsupported(arg, "a function of math of this type"));
            }
            _ if let Some(class) = class => Expr::FloatIs {
                class,
                operand: Box::new(float_value(operand, arg)?),
            },
            ("sqrt", _) => Expr::FloatSqrt {
                operand: Box::new(float_value(operand, arg)?),
                site,
            },
            (_, Rep::Int | Rep::Bool) => self.int_value(operand, arg)?,
            (_, Rep::Float) => {
                let operand = Box::new(operand);
                let whole = match member {
                    "floor" => Expr::FloatFloor(operand),
                    "ceil" => Expr::FloatCeil(operand),
                    // `int` of a `float` drops its fraction.
                    _ => *operand,
                };
                Expr::FloatAsInt {
                    operand: Box::new(whole),
                    site,
                }
            }
        };
        self.after_import(imported, value)
    }
}
