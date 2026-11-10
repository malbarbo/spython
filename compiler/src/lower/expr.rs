//! Expressions and their operators.

use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::{Lowering, coerce, join, unsupported};
use crate::Error;
use crate::ir::{ArithOp, CmpOp, DivOp, Expr, FloatOp, FuncId, LogicOp, MinMaxOp, Rep};

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
                self.check_surrogate(expr, value)?;
                Ok(Expr::Str(value.to_owned()))
            }
            ast::Expr::FString(f) => self.fstring(f),
            ast::Expr::NumberLiteral(n) => match &n.value {
                ast::Number::Int(i) => int_literal(expr, i.as_i64()),
                ast::Number::Float(x) => Ok(Expr::Float(*x)),
                _ => Err(unsupported(expr, "this number")),
            },
            ast::Expr::BooleanLiteral(b) => Ok(Expr::Bool(b.value)),
            ast::Expr::NoneLiteral(_) => Ok(Expr::None),
            ast::Expr::Name(_) | ast::Expr::Attribute(_) => match self.math_member(expr) {
                Some((imported, member)) => self.math_constant(expr, imported, &member),
                None => match expr {
                    ast::Expr::Name(name) => self.load(name),
                    ast::Expr::Attribute(attr) => match &*attr.value {
                        ast::Expr::Name(name) if let Some(class) = self.class_named(name) => {
                            self.class_attribute(attr, name, class)
                        }
                        value => match self.rep_of(expr_shape(self.model(), value)) {
                            Some(Rep::Object(class)) => {
                                let object = self.expr(value)?;
                                let field = self.object_attribute(attr, object, class)?;
                                self.narrowed(expr, field)
                            }
                            _ => Err(unsupported(expr, "this expression")),
                        },
                    },
                    _ => Err(unsupported(expr, "this expression")),
                },
            },
            ast::Expr::UnaryOp(u) => self.unary(u),
            ast::Expr::BinOp(b) => {
                let left = self.expr(&b.left)?;
                let right = self.expr(&b.right)?;
                self.binary(b, b.op, (left, &*b.left), (right, &*b.right))
            }
            ast::Expr::BoolOp(b) => self.bool_op(b, Self::logic_operand),
            ast::Expr::If(i) => {
                let cond = self.truth_of(&i.test)?;
                let then = self.expr(&i.body)?;
                let else_ = self.expr(&i.orelse)?;
                let rep = join(then.rep(), else_.rep());
                let widen =
                    |value| Box::new(coerce(value, rep).expect("the join holds both values"));
                Ok(Expr::IfElse {
                    cond: Box::new(cond),
                    then: widen(then),
                    else_: widen(else_),
                })
            }
            ast::Expr::Compare(c) => self.compare(c),
            ast::Expr::Subscript(sub) => {
                let value = self.expr(&sub.value)?;
                if value.rep() != Rep::Str {
                    return Err(unsupported(&*sub.value, "a subscript of this type"));
                }
                if let ast::Expr::Slice(slice) = &*sub.slice {
                    // `None` stands for a bound that the slice leaves out.
                    let bound = |bound: &Option<Box<ast::Expr>>| match bound.as_deref() {
                        None | Some(ast::Expr::NoneLiteral(_)) => Ok(None),
                        Some(bound) => self.int_operand(bound).map(Box::new).map(Some),
                    };
                    if slice.step.is_some() {
                        let args = [&slice.lower, &slice.upper, &slice.step]
                            .into_iter()
                            .map(|part| {
                                let part = bound(part)?.map_or(Expr::None, |part| *part);
                                Ok(coerce(part, Rep::Uniform)
                                    .expect("a uniform value holds any value"))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        let args = std::iter::once(value).chain(args).collect();
                        return self.prelude_call(sub, "str", "slice_step", args);
                    }
                    return Ok(Expr::Slice {
                        value: Box::new(value),
                        start: bound(&slice.lower)?,
                        stop: bound(&slice.upper)?,
                        site: self.site(sub.range()),
                    });
                }
                let index = self.int_operand(&sub.slice)?;
                Ok(Expr::Index {
                    value: Box::new(value),
                    index: Box::new(index),
                    site: self.site(sub.range()),
                })
            }
            ast::Expr::Call(call) => match self.builtin_name(&call.func).as_deref() {
                Some("print") => self.print(call),
                Some("len") => self.len(call),
                Some("abs") => self.abs(call),
                Some("pow") => self.pow(call),
                Some("round") => self.round(call),
                Some("bool") => self.bool(call),
                Some("float") => self.float(call),
                Some("int") => self.int(call),
                Some("str") => self.str(call),
                Some("repr") => self.repr(call),
                Some("ord") => self.ord(call),
                Some("chr") => self.chr(call),
                Some("min") => self.min_max(call, MinMaxOp::Min),
                Some("max") => self.min_max(call, MinMaxOp::Max),
                _ => match self.math_member(&call.func) {
                    Some((imported, member)) => self.math_call(call, imported, &member),
                    None => self.call(call),
                },
            },
            _ => Err(unsupported(expr, "this expression")),
        }
    }

    /// Refuses the text `value` of the literal `node` when it may hold a
    /// surrogate. The parser reads the escape of a lone surrogate, as in
    /// "\ud800", as U+FFFD. Python keeps the surrogate, and printing it
    /// raises UnicodeEncodeError.
    fn check_surrogate(&self, node: &impl Ranged, value: &str) -> Result<(), Error> {
        let source = self
            .source()
            .get(std::ops::Range::<usize>::from(node.range()));
        if value.contains('\u{fffd}') && source.is_none_or(|text| text.contains('\\')) {
            return Err(unsupported(
                node,
                "a str with an escape that may be a surrogate",
            ));
        }
        Ok(())
    }

    /// An f-string where no replacement field has the conversion `!a`, a
    /// format or an `=`.
    fn fstring(&self, node: &ast::ExprFString) -> Result<Expr, Error> {
        let mut parts: Vec<Expr> = Vec::new();
        // Python joins the literals that come one after the other.
        let literal = |parts: &mut Vec<Expr>, text: &str| match parts.last_mut() {
            Some(Expr::Str(last)) => last.push_str(text),
            _ => parts.push(Expr::Str(text.to_owned())),
        };
        for part in &node.value {
            match part {
                ast::FStringPartRef::Literal(s) => {
                    self.check_surrogate(node, &s.value)?;
                    literal(&mut parts, &s.value);
                }
                ast::FStringPartRef::FString(f) => {
                    for element in &f.elements {
                        match element {
                            ast::InterpolatedStringElement::Literal(l) => {
                                self.check_surrogate(node, &l.value)?;
                                literal(&mut parts, &l.value);
                            }
                            ast::InterpolatedStringElement::Interpolation(field) => {
                                let plain = field.debug_text.is_none()
                                    && field.format_spec.is_none()
                                    && field.conversion != ast::ConversionFlag::Ascii;
                                if !plain {
                                    return Err(unsupported(
                                        field,
                                        "a replacement field with a conversion or a format",
                                    ));
                                }
                                let value = self.expr(&field.expression)?;
                                let text = if field.conversion == ast::ConversionFlag::Repr {
                                    self.repr_of(value, &*field.expression, field.range())?
                                } else {
                                    self.str_of(value, &*field.expression, field.range())?
                                };
                                match text {
                                    Expr::Str(text) => literal(&mut parts, &text),
                                    value => parts.push(value),
                                }
                            }
                        }
                    }
                }
            }
        }
        if parts.len() > 1 {
            return Ok(Expr::Join {
                parts,
                site: self.site(node.range()),
            });
        }
        Ok(parts.pop().unwrap_or_else(|| Expr::Str(String::new())))
    }

    /// The `str` of `lowered`, the value of `node`. The conversion covers
    /// `range`.
    pub(super) fn str_of(
        &self,
        lowered: Expr,
        node: &impl Ranged,
        range: TextRange,
    ) -> Result<Expr, Error> {
        match lowered.rep() {
            Rep::Str => Ok(lowered),
            Rep::Int | Rep::Float | Rep::Bool => Ok(Expr::StrOf {
                operand: Box::new(lowered),
                site: self.site(range),
            }),
            Rep::None => Err(unsupported(node, "str of None")),
            Rep::StrOrNone | Rep::Uniform => Err(unsupported(node, "str of a union")),
            Rep::Object(_) | Rep::ObjectOrNone(_) => {
                let func = self
                    .object_str(lowered.rep())?
                    .expect("the str of an object");
                Ok(self.class_call(func, lowered, range))
            }
        }
    }

    /// The function that gives the `str` of a value of `rep`, for an object
    /// or an object or `None`, and `None` for the other reps.
    pub(super) fn object_str(&self, rep: Rep) -> Result<Option<FuncId>, Error> {
        match rep {
            Rep::Object(class) => self.class_str(class).map(Some),
            Rep::ObjectOrNone(class) => self.or_none_conversion(class, false).map(Some),
            _ => Ok(None),
        }
    }

    /// A chain of comparisons, where every operand is a `str`, or every
    /// operand is a number. When one operand is a `float`, every operand
    /// becomes a `float`, which is exact for an `int`.
    fn compare(&self, c: &ast::ExprCompare) -> Result<Expr, Error> {
        if let ([op @ (ast::CmpOp::Is | ast::CmpOp::IsNot)], [left, right]) =
            (&*c.ops, &*c.operands)
        {
            let is = match (left, right) {
                (ast::Expr::NoneLiteral(_), value) | (value, ast::Expr::NoneLiteral(_)) => {
                    Expr::IsNone(Box::new(self.expr(value)?))
                }
                _ => {
                    let left = self.expr(left)?;
                    let right = self.expr(right)?;
                    // A value that the checker narrows to `None` has no
                    // reference, and gets the null of the other class.
                    let reference = |value: Expr, other: Rep| match (value.rep(), other) {
                        (rep, _) if is_object(rep) => Some(value),
                        (Rep::None, Rep::Object(class) | Rep::ObjectOrNone(class)) => {
                            coerce(value, Rep::ObjectOrNone(class))
                        }
                        _ => None,
                    };
                    let (left_rep, right_rep) = (left.rep(), right.rep());
                    match (reference(left, right_rep), reference(right, left_rep)) {
                        (Some(left), Some(right)) => Expr::Is(Box::new(left), Box::new(right)),
                        _ => {
                            return Err(unsupported(c, "is between values that are not objects"));
                        }
                    }
                }
            };
            return Ok(match op {
                ast::CmpOp::Is => is,
                _ => Expr::Not(Box::new(is)),
            });
        }
        if let ([op @ (ast::CmpOp::In | ast::CmpOp::NotIn)], [left, right]) =
            (&*c.ops, &*c.operands)
        {
            let sub = self.expr(left)?;
            let s = self.expr(right)?;
            if sub.rep() != Rep::Str || s.rep() != Rep::Str {
                return Err(unsupported(c, "in between values that are not strs"));
            }
            let is_in = self.prelude_call(c, "str", "is_in", vec![sub, s])?;
            return Ok(match op {
                ast::CmpOp::In => is_in,
                _ => Expr::Not(Box::new(is_in)),
            });
        }
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
        if operands
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if self.share_dataclass(left, right)))
        {
            return self.dataclass_compare(c, ops, operands);
        }
        // A member of an `Enum` is equal to itself alone, and an object of
        // a dataclass is not equal to a value of another class or type.
        if operands.iter().any(|(lowered, _)| is_object(lowered.rep())) {
            if ops.iter().any(|op| !matches!(op, CmpOp::Eq | CmpOp::Ne)) {
                return Err(unsupported(c, "an order of objects"));
            }
            // The tests of the type of a uniform value compare an object
            // with a value of another class or type.
            let rep = operands
                .iter()
                .map(|(lowered, _)| lowered.rep())
                .reduce(join)
                .expect("a comparison has a first operand");
            let mut operands = operands
                .into_iter()
                .map(|(lowered, _)| coerce(lowered, rep).expect("the join holds every operand"));
            let first = operands.next().expect("a comparison has a first operand");
            return Ok(Expr::Compare(
                Box::new(first),
                ops.into_iter().zip(operands).collect(),
            ));
        }
        // A union compares by the types of its values, which the module
        // tests when it runs.
        let union = |(lowered, _): &(Expr, _)| {
            matches!(lowered.rep(), Rep::None | Rep::StrOrNone | Rep::Uniform)
        };
        if operands.iter().any(union) {
            let mut operands = operands.into_iter().map(|(lowered, _)| {
                coerce(lowered, Rep::Uniform).expect("a uniform value holds any value")
            });
            let first = operands.next().expect("a comparison has a first operand");
            return Ok(Expr::Compare(
                Box::new(first),
                ops.into_iter().zip(operands).collect(),
            ));
        }
        let is_str = |(lowered, _): &(Expr, _)| lowered.rep() == Rep::Str;
        let is_float = |(lowered, _): &(Expr, _)| lowered.rep() == Rep::Float;
        let mut operands = if operands.iter().any(is_float) && !operands.iter().any(is_str) {
            operands
                .into_iter()
                .map(|(lowered, operand)| float_value(lowered, operand))
                .collect::<Result<Vec<_>, _>>()?
        } else if !operands.iter().any(is_str) {
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
        (right, right_node): (Expr, &ast::Expr),
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
        let floats = left.rep() == Rep::Float || right.rep() == Rep::Float;
        if op == ast::Operator::Div {
            return Ok(Expr::Div {
                op: DivOp::True { ints: !floats },
                left: Box::new(float_value(left, left_node)?),
                right: Box::new(float_value(right, right_node)?),
                site: self.site(node.range()),
            });
        }
        if op == ast::Operator::Pow && !floats {
            return self.int_power(node, (left, left_node), (right, right_node));
        }
        if op == ast::Operator::Pow && floats {
            return Ok(Expr::FloatPow {
                left: Box::new(float_value(left, left_node)?),
                right: Box::new(float_value(right, right_node)?),
                site: self.site(node.range()),
            });
        }
        let op = arith_op(op).ok_or_else(|| unsupported(node, "this operator"))?;
        if floats {
            let left = Box::new(float_value(left, left_node)?);
            let right = Box::new(float_value(right, right_node)?);
            let float_op = match op {
                ArithOp::Add => FloatOp::Add,
                ArithOp::Sub => FloatOp::Sub,
                ArithOp::Mul => FloatOp::Mul,
                ArithOp::FloorDiv | ArithOp::Mod => {
                    return Ok(Expr::Div {
                        op: if op == ArithOp::Mod {
                            DivOp::Mod
                        } else {
                            DivOp::Floor
                        },
                        left,
                        right,
                        site: self.site(node.range()),
                    });
                }
            };
            return Ok(Expr::FloatArith {
                op: float_op,
                left,
                right,
            });
        }
        Ok(Expr::Arith {
            op,
            left: Box::new(self.int_value(left, left_node)?),
            right: Box::new(self.int_value(right, right_node)?),
            site: self.site(node.range()),
        })
    }

    /// `left ** right` of two `int`s. The type of the exponent gives the
    /// type of the power, as the checker does. A negative literal gives a
    /// `float`, a literal that is not negative gives an `int`, and any
    /// other `int` gives an `int` or a `float`.
    fn int_power(
        &self,
        node: &impl Ranged,
        (left, left_node): (Expr, &impl Ranged),
        (right, right_node): (Expr, &ast::Expr),
    ) -> Result<Expr, Error> {
        // The sign of the exponent, when its type tells it.
        let sign = |shape: &Shape| match shape {
            Shape::IntLit(exp) if *exp < 0 => Some(true),
            Shape::IntLit(_) | Shape::BoolLit(_) | Shape::Bool => Some(false),
            _ => None,
        };
        let negative = match expr_shape(self.model(), right_node) {
            Some(Shape::Union(shapes)) => {
                let signs: Option<Vec<_>> = shapes.iter().map(sign).collect();
                signs.and_then(|signs| {
                    let first = *signs.first()?;
                    signs.iter().all(|sign| *sign == first).then_some(first)
                })
            }
            shape => shape.as_ref().and_then(sign),
        };
        if negative == Some(true) {
            return Ok(Expr::FloatPow {
                left: Box::new(float_value(left, left_node)?),
                right: Box::new(float_value(right, right_node)?),
                site: self.site(node.range()),
            });
        }
        let base = self.int_value(left, left_node)?;
        let exp = self.int_value(right, right_node)?;
        if negative == Some(false) {
            return Ok(Expr::IntPow {
                base: Box::new(base),
                exp: Box::new(exp),
                site: self.site(node.range()),
            });
        }
        self.prelude_call(node, "int", "power", vec![base, exp])
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
                    let lowered = self.expr(operand)?;
                    if lowered.rep() == Rep::Float {
                        return Ok(Expr::FloatNeg(Box::new(lowered)));
                    }
                    Ok(Expr::Arith {
                        op: ArithOp::Sub,
                        left: Box::new(Expr::Int(0)),
                        right: Box::new(self.int_value(lowered, operand)?),
                        site: self.site(u.range()),
                    })
                }
            },
            ast::UnaryOp::UAdd => {
                let lowered = self.expr(&u.operand)?;
                if lowered.rep() == Rep::Float {
                    return Ok(lowered);
                }
                self.int_value(lowered, &*u.operand)
            }
            ast::UnaryOp::Not => Ok(Expr::Not(Box::new(self.truth_of(&u.operand)?))),
            ast::UnaryOp::Invert => Err(unsupported(u, "this operator")),
        }
    }

    /// An operand of `and` or `or`, which has to have a truth.
    fn logic_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(operand)?;
        match lowered.rep() {
            Rep::None => Err(unsupported(operand, "the truth of None")),
            Rep::StrOrNone | Rep::Uniform => Err(unsupported(operand, "the truth of a union")),
            Rep::Int
            | Rep::Float
            | Rep::Bool
            | Rep::Str
            | Rep::Object(_)
            | Rep::ObjectOrNone(_) => Ok(lowered),
        }
    }

    /// The operand of an operator of `int`, which may be a `bool`.
    pub(super) fn int_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        self.int_value(self.expr(operand)?, operand)
    }

    /// `lowered`, the value of `operand`, as an `int`.
    pub(super) fn int_value(&self, lowered: Expr, operand: &impl Ranged) -> Result<Expr, Error> {
        match lowered.rep() {
            Rep::Int => Ok(lowered),
            Rep::Bool => Ok(Expr::BoolAsInt(Box::new(lowered))),
            Rep::Float
            | Rep::Str
            | Rep::None
            | Rep::StrOrNone
            | Rep::Uniform
            | Rep::Object(_)
            | Rep::ObjectOrNone(_) => Err(unsupported(
                operand,
                "an operator on a value that is not an int",
            )),
        }
    }
}

/// Returns `true` if a value of `rep` is an object of a class, or `None`
/// in the place of one, `false` otherwise.
fn is_object(rep: Rep) -> bool {
    matches!(rep, Rep::Object(_) | Rep::ObjectOrNone(_))
}

/// `lowered`, the value of `operand`, as a `float`.
pub(super) fn float_value(lowered: Expr, operand: &impl Ranged) -> Result<Expr, Error> {
    match lowered.rep() {
        Rep::Float => Ok(lowered),
        Rep::Int => Ok(Expr::IntAsFloat(Box::new(lowered))),
        Rep::Bool => Ok(Expr::IntAsFloat(Box::new(Expr::BoolAsInt(Box::new(
            lowered,
        ))))),
        Rep::Str
        | Rep::None
        | Rep::StrOrNone
        | Rep::Uniform
        | Rep::Object(_)
        | Rep::ObjectOrNone(_) => Err(unsupported(
            operand,
            "an operator on a value that is not a number",
        )),
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
        // An object is true, since neither form of a class defines
        // `__bool__` or `__len__`, and an object or `None` is true when it
        // is an object.
        Rep::Int | Rep::Float | Rep::Str | Rep::Object(_) | Rep::ObjectOrNone(_) => {
            Ok(Expr::Truth(Box::new(lowered)))
        }
        Rep::None => Err(unsupported(node, "the truth of None")),
        Rep::StrOrNone | Rep::Uniform => Err(unsupported(node, "the truth of a union")),
    }
}

/// The `int` of a literal, which has to fit in 31 bits.
pub(super) fn int_literal(node: &impl Ranged, value: Option<i64>) -> Result<Expr, Error> {
    match value {
        Some(v) if (INT_MIN..=INT_MAX).contains(&v) => {
            Ok(Expr::Int(i32::try_from(v).expect("31 bits fit in an i32")))
        }
        _ => Err(unsupported(node, "an int that does not fit in 31 bits")),
    }
}
