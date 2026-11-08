//! Expressions and their operators.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{ArithOp, CmpOp, Expr, LogicOp, Rep};

/// The range of an `int`, which has to fit in 31 bits.
const INT_MIN: i64 = -(1 << 30);
const INT_MAX: i64 = (1 << 30) - 1;

impl<'a> Lowering<'a, '_> {
    /// The truth of `expr` as a `bool`, where only the truth matters, as
    /// in a condition. The truth of `a and b` is the truth of `a` and the
    /// truth of `b`, so `a` and `b` may have two types.
    pub(super) fn truth_of(&self, expr: &ast::Expr) -> Result<Expr, Error> {
        match expr {
            ast::Expr::BoolOp(b) => self.bool_op(b, Self::truth_of),
            ast::Expr::UnaryOp(ast::ExprUnaryOp {
                op: ast::UnaryOp::Not,
                operand,
                ..
            }) => Ok(Expr::Not(Box::new(self.truth_of(operand)?))),
            _ => truth(self.expr(expr)?, expr),
        }
    }

    /// `and` or `or`, with each operand lowered by `operand`.
    fn bool_op(
        &self,
        b: &ast::ExprBoolOp,
        operand: impl Fn(&Self, &ast::Expr) -> Result<Expr, Error>,
    ) -> Result<Expr, Error> {
        let op = match b.op {
            ast::BoolOp::And => LogicOp::And,
            ast::BoolOp::Or => LogicOp::Or,
        };
        let (first, rest) = b
            .values
            .split_first()
            .expect("a bool operator has operands");
        let first = operand(self, first)?;
        let rep = first.rep();
        // `a and b and c` gives the value of `(a and b) and c`, and
        // evaluates the same operands.
        rest.iter().try_fold(first, |left, right| {
            let lowered = operand(self, right)?;
            if lowered.rep() != rep {
                return Err(unsupported(b, "operands of two types"));
            }
            Ok(Expr::Logic {
                op,
                left: Box::new(left),
                right: Box::new(lowered),
            })
        })
    }

    pub(super) fn expr(&self, expr: &ast::Expr) -> Result<Expr, Error> {
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
            ast::Expr::Name(name) => self.load(name),
            ast::Expr::UnaryOp(u) => self.unary(u),
            ast::Expr::BinOp(b) => {
                let left = self.expr(&b.left)?;
                let right = self.expr(&b.right)?;
                self.binary(b, b.op, (left, &*b.left), (right, &*b.right))
            }
            ast::Expr::BoolOp(b) => self.bool_op(b, Self::logic_operand),
            ast::Expr::Compare(c) => self.compare(c),
            ast::Expr::Subscript(sub) => {
                let value = self.expr(&sub.value)?;
                if value.rep() != Rep::Str {
                    return Err(unsupported(&*sub.value, "a subscript of this type"));
                }
                if matches!(&*sub.slice, ast::Expr::Slice(_)) {
                    return Err(unsupported(&*sub.slice, "a slice"));
                }
                let index = self.int_operand(&sub.slice)?;
                Ok(Expr::Index {
                    value: Box::new(value),
                    index: Box::new(index),
                    site: self.site(sub.range()),
                })
            }
            ast::Expr::Call(call) if self.is_builtin(&call.func, "print") => self.print(call),
            ast::Expr::Call(call) if self.is_builtin(&call.func, "len") => self.len(call),
            ast::Expr::Call(call) => self.call(call),
            _ => Err(unsupported(expr, "this expression")),
        }
    }

    /// A chain of comparisons, where every operand is a `str`, or every
    /// operand is an `int` or a `bool`.
    fn compare(&self, c: &ast::ExprCompare) -> Result<Expr, Error> {
        let ops = c
            .ops
            .iter()
            .map(|op| match op {
                ast::CmpOp::Eq => Ok(CmpOp::Eq),
                ast::CmpOp::NotEq => Ok(CmpOp::Ne),
                ast::CmpOp::Lt => Ok(CmpOp::Lt),
                ast::CmpOp::LtE => Ok(CmpOp::Le),
                ast::CmpOp::Gt => Ok(CmpOp::Gt),
                ast::CmpOp::GtE => Ok(CmpOp::Ge),
                _ => Err(unsupported(c, "this comparison")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let operands = std::iter::once(c.first_operand())
            .chain(c.comparators())
            .map(|operand| Ok((self.expr(operand)?, operand)))
            .collect::<Result<Vec<_>, Error>>()?;
        let is_str = |(lowered, _): &(Expr, _)| lowered.rep() == Rep::Str;
        let mut operands = if !operands.iter().any(is_str) {
            operands
                .into_iter()
                .map(|(lowered, operand)| self.int_value(lowered, operand))
                .collect::<Result<Vec<_>, _>>()?
        } else if operands.iter().all(is_str) {
            operands.into_iter().map(|(lowered, _)| lowered).collect()
        } else {
            return Err(unsupported(c, "a comparison of two types"));
        }
        .into_iter();
        let first = operands.next().expect("a comparison has a first operand");
        Ok(Expr::Compare(
            Box::new(first),
            ops.into_iter().zip(operands).collect(),
        ))
    }

    /// The operator `op` of `node` on two lowered operands, each with the
    /// node it comes from.
    pub(super) fn binary(
        &self,
        node: &impl Ranged,
        op: ast::Operator,
        (left, left_node): (Expr, &impl Ranged),
        (right, right_node): (Expr, &impl Ranged),
    ) -> Result<Expr, Error> {
        match (op, left.rep(), right.rep()) {
            (ast::Operator::Add, Rep::Str, Rep::Str) => {
                return Ok(Expr::Concat {
                    left: Box::new(left),
                    right: Box::new(right),
                    site: self.site(node.range()),
                });
            }
            (ast::Operator::Mult, Rep::Str, _) => {
                return Ok(Expr::Repeat {
                    left: Box::new(left),
                    right: Box::new(self.int_value(right, right_node)?),
                    site: self.site(node.range()),
                });
            }
            (ast::Operator::Mult, _, Rep::Str) => {
                return Ok(Expr::Repeat {
                    left: Box::new(self.int_value(left, left_node)?),
                    right: Box::new(right),
                    site: self.site(node.range()),
                });
            }
            (_, Rep::Str, _) | (_, _, Rep::Str) => return Err(unsupported(node, "this operator")),
            _ => {}
        }
        let op = arith_op(op).ok_or_else(|| unsupported(node, "this operator"))?;
        Ok(Expr::Arith {
            op,
            left: Box::new(self.int_value(left, left_node)?),
            right: Box::new(self.int_value(right, right_node)?),
            site: self.site(node.range()),
        })
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
                    Ok(Expr::Arith {
                        op: ArithOp::Sub,
                        left: Box::new(Expr::Int(0)),
                        right: Box::new(operand),
                        site: self.site(u.range()),
                    })
                }
            },
            ast::UnaryOp::UAdd => self.int_operand(&u.operand),
            ast::UnaryOp::Not => Ok(Expr::Not(Box::new(self.truth_of(&u.operand)?))),
            ast::UnaryOp::Invert => Err(unsupported(u, "this operator")),
        }
    }

    /// An operand of `and` or `or`, which has to have a truth.
    fn logic_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(operand)?;
        match lowered.rep() {
            Rep::None => Err(unsupported(operand, "the truth of None")),
            _ => Ok(lowered),
        }
    }

    /// The operand of an operator of `int`, which may be a `bool`.
    fn int_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        self.int_value(self.expr(operand)?, operand)
    }

    /// `lowered`, the value of `operand`, as an `int`.
    fn int_value(&self, lowered: Expr, operand: &impl Ranged) -> Result<Expr, Error> {
        match lowered.rep() {
            Rep::Int => Ok(lowered),
            Rep::Bool => Ok(Expr::BoolAsInt(Box::new(lowered))),
            Rep::Str | Rep::None => Err(unsupported(
                operand,
                "an operator on a value that is not an int",
            )),
        }
    }
}

fn arith_op(op: ast::Operator) -> Option<ArithOp> {
    match op {
        ast::Operator::Add => Some(ArithOp::Add),
        ast::Operator::Sub => Some(ArithOp::Sub),
        ast::Operator::Mult => Some(ArithOp::Mul),
        ast::Operator::FloorDiv => Some(ArithOp::FloorDiv),
        ast::Operator::Mod => Some(ArithOp::Mod),
        _ => None,
    }
}

/// The truth of `lowered`, the value of `node`, as a `bool`.
pub(super) fn truth(lowered: Expr, node: &impl Ranged) -> Result<Expr, Error> {
    match lowered.rep() {
        Rep::Bool => Ok(lowered),
        Rep::Int | Rep::Str => Ok(Expr::Truth(Box::new(lowered))),
        Rep::None => Err(unsupported(node, "the truth of None")),
    }
}

/// The `int` of a literal, which has to fit in 31 bits.
fn int_literal(node: &impl Ranged, value: Option<i64>) -> Result<Expr, Error> {
    match value {
        Some(v) if (INT_MIN..=INT_MAX).contains(&v) => {
            Ok(Expr::Int(i32::try_from(v).expect("31 bits fit in an i32")))
        }
        _ => Err(unsupported(node, "an int that does not fit in 31 bits")),
    }
}
