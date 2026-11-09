//! The names of `math`, which an import binds.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, Rep};

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
    /// `math.floor`, `math.ceil` and `math.trunc` take one number and give
    /// an `int`.
    pub(super) fn math_call(
        &self,
        call: &ast::ExprCall,
        imported: &ast::ExprName,
        member: &str,
    ) -> Result<Expr, Error> {
        let op: fn(Box<Expr>) -> Expr = match member {
            "floor" => Expr::FloatFloor,
            "ceil" => Expr::FloatCeil,
            // `int` of a `float` drops its fraction.
            "trunc" => |operand| *operand,
            _ => return Err(unsupported(call, "this function of math")),
        };
        let arg = match (&*call.arguments.args, &*call.arguments.keywords) {
            ([arg], []) => arg,
            _ => return Err(unsupported(call, "this call of a function of math")),
        };
        let operand = self.expr(arg)?;
        let value = match operand.rep() {
            Rep::Int | Rep::Bool => self.int_value(operand, arg)?,
            Rep::Float => Expr::FloatAsInt {
                operand: Box::new(op(Box::new(operand))),
                site: self.site(call.range()),
            },
            Rep::Str | Rep::None => {
                return Err(unsupported(arg, "a function of math of this type"));
            }
        };
        self.after_import(imported, value)
    }
}
