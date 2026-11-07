//! From the syntax tree and the types of the checker to the IR.

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::types::lowering::{Shape, expr_shape};
use ty_python_semantic::{Db, SemanticModel};

use crate::Error;
use crate::ir::{ArithOp, CmpOp, Expr, Program, Rep, Stmt};

/// The range of an `int`, which has to fit in 31 bits.
const INT_MIN: i64 = -(1 << 30);
const INT_MAX: i64 = (1 << 30) - 1;

pub(crate) fn lower(db: &dyn Db, file: File) -> Result<Program, Error> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let source = source_text(db, file);
    let lowering = Lowering {
        model: &model,
        source: source.as_str(),
    };
    let body = module
        .suite()
        .iter()
        .map(|stmt| lowering.stmt(stmt))
        .collect::<Result<_, _>>()?;
    Ok(Program { body })
}

struct Lowering<'a, 'db> {
    model: &'a SemanticModel<'db>,
    source: &'a str,
}

impl Lowering<'_, '_> {
    fn stmt(&self, stmt: &ast::Stmt) -> Result<Stmt, Error> {
        match stmt {
            ast::Stmt::Expr(e) => Ok(Stmt::Expr(self.expr(&e.value)?)),
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    fn expr(&self, expr: &ast::Expr) -> Result<Expr, Error> {
        match expr {
            ast::Expr::StringLiteral(s) => {
                let value = s.value.to_str();
                // The parser reads the escape of a lone surrogate, as in
                // "\ud800", as U+FFFD. Python keeps the surrogate, and printing
                // it raises UnicodeEncodeError.
                let source = self
                    .source
                    .get(std::ops::Range::<usize>::from(expr.range()));
                if value.contains('\u{fffd}') && source.is_none_or(|text| text.contains('\\')) {
                    return Err(unsupported(
                        expr,
                        "a str with an escape that may be a surrogate",
                    ));
                }
                Ok(Expr::Str(value.to_owned()))
            }
            ast::Expr::NumberLiteral(n) => match &n.value {
                ast::Number::Int(i) => int_literal(expr, i.as_i64()),
                _ => Err(unsupported(expr, "this number")),
            },
            ast::Expr::BooleanLiteral(b) => Ok(Expr::Bool(b.value)),
            ast::Expr::UnaryOp(u) => self.unary(u),
            ast::Expr::BinOp(b) => {
                let op = match b.op {
                    ast::Operator::Add => ArithOp::Add,
                    ast::Operator::Sub => ArithOp::Sub,
                    ast::Operator::Mult => ArithOp::Mul,
                    _ => return Err(unsupported(expr, "this operator")),
                };
                let left = self.int_operand(&b.left)?;
                let right = self.int_operand(&b.right)?;
                Ok(Expr::Arith(op, Box::new(left), Box::new(right)))
            }
            ast::Expr::Compare(c) => {
                let first = self.int_operand(c.first_operand())?;
                let rest = c
                    .ops
                    .iter()
                    .zip(c.comparators())
                    .map(|(op, right)| {
                        let op = match op {
                            ast::CmpOp::Eq => CmpOp::Eq,
                            ast::CmpOp::NotEq => CmpOp::Ne,
                            ast::CmpOp::Lt => CmpOp::Lt,
                            ast::CmpOp::LtE => CmpOp::Le,
                            ast::CmpOp::Gt => CmpOp::Gt,
                            ast::CmpOp::GtE => CmpOp::Ge,
                            _ => return Err(unsupported(expr, "this comparison")),
                        };
                        Ok((op, self.int_operand(right)?))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Expr::Compare(Box::new(first), rest))
            }
            ast::Expr::Call(call) if self.is_builtin_print(&call.func) => self.print(call),
            _ => Err(unsupported(expr, "this expression")),
        }
    }

    fn unary(&self, u: &ast::ExprUnaryOp) -> Result<Expr, Error> {
        match u.op {
            // The literal alone may not fit, as in `-1073741824`.
            ast::UnaryOp::USub => match &*u.operand {
                ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
                    value: ast::Number::Int(i),
                    ..
                }) => int_literal(u, i.as_i64().map(|i| -i)),
                operand => {
                    let operand = self.int_operand(operand)?;
                    Ok(Expr::Arith(
                        ArithOp::Sub,
                        Box::new(Expr::Int(0)),
                        Box::new(operand),
                    ))
                }
            },
            ast::UnaryOp::UAdd => self.int_operand(&u.operand),
            _ => Err(unsupported(u, "this operator")),
        }
    }

    /// The operand of an operator of `int`, which may be a `bool`.
    fn int_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(operand)?;
        match lowered.rep() {
            Rep::Int => Ok(lowered),
            Rep::Bool => Ok(Expr::BoolAsInt(Box::new(lowered))),
            Rep::Str | Rep::None => Err(unsupported(
                operand,
                "an operator on a value that is not an int",
            )),
        }
    }

    fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::None => Err(unsupported(arg, "print of None")),
                    _ => Ok(lowered),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print(args))
    }

    /// Returns `true` if `func` is the `print` of the builtins, `false`
    /// otherwise.
    fn is_builtin_print(&self, func: &ast::Expr) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Function(def)) if def.name == "print" && is_builtins(self.model.db(), def.file)
        )
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}

/// The `int` of a literal, which has to fit in 31 bits.
fn int_literal(node: &impl Ranged, value: Option<i64>) -> Result<Expr, Error> {
    value
        .filter(|v| (INT_MIN..=INT_MAX).contains(v))
        .and_then(|v| i32::try_from(v).ok())
        .map(Expr::Int)
        .ok_or_else(|| unsupported(node, "an int that does not fit in 31 bits"))
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported {
        range: node.range(),
        what: what.to_owned(),
    }
}
