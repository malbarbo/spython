//! From the syntax tree and the types of the checker to the IR.

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::types::lowering::{
    Shape, expr_shape, is_definitely_bound, is_implicit_global,
};
use ty_python_semantic::{Db, SemanticModel};

use crate::Error;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::ir::{ArithOp, CmpOp, Expr, LogicOp, Program, Rep, Site, Stmt, Var, VarId};
use crate::traceback;

/// The range of an `int`, which has to fit in 31 bits.
const INT_MIN: i64 = -(1 << 30);
const INT_MAX: i64 = (1 << 30) - 1;

pub(crate) fn lower(db: &dyn Db, file: File, path: &str) -> Result<Program, Error> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let source = source_text(db, file);
    let lowering = Lowering {
        model: &model,
        source: source.as_str(),
        path,
        sites: RefCell::new(Vec::new()),
        vars: RefCell::new(Vars::default()),
    };
    let suite = module.suite().as_slice();
    lowering.declare_vars(suite)?;
    let body = lowering.body(suite)?;
    Ok(Program {
        body,
        vars: lowering.vars.into_inner().list,
        sites: lowering.sites.into_inner(),
    })
}

struct Lowering<'a, 'db> {
    model: &'a SemanticModel<'db>,
    source: &'a str,
    /// The path of the file in a traceback.
    path: &'a str,
    sites: RefCell<Vec<String>>,
    vars: RefCell<Vars>,
}

#[derive(Default)]
struct Vars {
    ids: HashMap<String, VarId>,
    list: Vec<Var>,
}

impl Vars {
    /// The variable of `name`, and its rep.
    fn find(&self, name: &str) -> Option<(VarId, Rep)> {
        let id = *self.ids.get(name)?;
        Some((id, self.get(id).rep))
    }

    fn get(&self, id: VarId) -> &Var {
        self.list
            .get(id.0 as usize)
            .expect("a variable of the module")
    }

    fn get_mut(&mut self, id: VarId) -> &mut Var {
        self.list
            .get_mut(id.0 as usize)
            .expect("a variable of the module")
    }
}

impl Lowering<'_, '_> {
    /// Gives each variable that the module assigns the rep of its values,
    /// which the checker infers. A read may come before the assignment, as
    /// in a loop, so every variable has a rep before the lowering starts.
    fn declare_vars(&self, body: &[ast::Stmt]) -> Result<(), Error> {
        for stmt in body {
            let (targets, value): (&[ast::Expr], &ast::Expr) = match stmt {
                ast::Stmt::If(i) => {
                    self.declare_vars(&i.body)?;
                    for clause in &i.elif_else_clauses {
                        self.declare_vars(&clause.body)?;
                    }
                    continue;
                }
                ast::Stmt::While(w) => {
                    self.declare_vars(&w.body)?;
                    self.declare_vars(&w.orelse)?;
                    continue;
                }
                ast::Stmt::Assign(a) => (&a.targets, &a.value),
                ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                    target,
                    value: Some(value),
                    ..
                }) => (std::slice::from_ref(target), value),
                _ => continue,
            };
            // The lowering of the assignment rejects a value without a rep.
            let Some(rep) = shape_rep(expr_shape(self.model, value)) else {
                continue;
            };
            for target in targets {
                if let ast::Expr::Name(name) = target {
                    self.declare_var(name.id.as_str(), rep, value)?;
                }
            }
        }
        Ok(())
    }

    fn declare_var(&self, name: &str, rep: Rep, value: &ast::Expr) -> Result<(), Error> {
        let mut vars = self.vars.borrow_mut();
        if let Some((_, declared)) = vars.find(name) {
            if declared != rep {
                return Err(unsupported(value, "a variable with values of two types"));
            }
            return Ok(());
        }
        let id = VarId(u32::try_from(vars.list.len()).expect("fewer than 2^32 variables"));
        vars.ids.insert(name.to_owned(), id);
        vars.list.push(Var {
            rep,
            undefined: format!("NameError: name '{name}' is not defined"),
            checked: false,
        });
        Ok(())
    }

    fn body(&self, body: &[ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            lowered.extend(self.stmt(stmt)?);
        }
        Ok(lowered)
    }

    fn stmt(&self, stmt: &ast::Stmt) -> Result<Option<Stmt>, Error> {
        match stmt {
            ast::Stmt::Expr(e) => Ok(Some(Stmt::Expr(self.expr(&e.value)?))),
            ast::Stmt::Assign(a) => {
                let value = self.expr(&a.value)?;
                let targets = a
                    .targets
                    .iter()
                    .map(|target| self.target(target, &value))
                    .collect::<Result<_, _>>()?;
                Ok(Some(Stmt::Assign { targets, value }))
            }
            // An annotation alone binds nothing.
            ast::Stmt::AnnAssign(ast::StmtAnnAssign { value: None, .. }) => Ok(None),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                let value = self.expr(value)?;
                let target = self.target(target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::AugAssign(a) => {
                let ast::Expr::Name(name) = &*a.target else {
                    return Err(unsupported(&*a.target, "this target"));
                };
                let left = self.int_value(self.load(name)?, name)?;
                let right = self.int_operand(&a.value)?;
                let value = Expr::Arith {
                    op: arith_op(a.op).ok_or_else(|| unsupported(a, "this operator"))?,
                    left: Box::new(left),
                    right: Box::new(right),
                    site: self.site(a.range()),
                };
                let target = self.target(&a.target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::If(i) => {
                let mut else_ = Vec::new();
                // Each `elif` is an `if` in the `else` of the clause before it.
                for clause in i.elif_else_clauses.iter().rev() {
                    let body = self.body(&clause.body)?;
                    else_ = match &clause.test {
                        Some(test) => vec![Stmt::If {
                            cond: self.cond(test)?,
                            then: body,
                            else_,
                        }],
                        None => body,
                    };
                }
                Ok(Some(Stmt::If {
                    cond: self.cond(&i.test)?,
                    then: self.body(&i.body)?,
                    else_,
                }))
            }
            ast::Stmt::While(w) => Ok(Some(Stmt::While {
                cond: self.cond(&w.test)?,
                body: self.body(&w.body)?,
                else_: self.body(&w.orelse)?,
            })),
            ast::Stmt::Break(_) => Ok(Some(Stmt::Break)),
            ast::Stmt::Continue(_) => Ok(Some(Stmt::Continue)),
            ast::Stmt::Pass(_) => Ok(None),
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    /// The condition of an `if` or a `while`.
    fn cond(&self, test: &ast::Expr) -> Result<Expr, Error> {
        truth(self.expr(test)?, test)
    }

    /// The variable that `target` names, which has to hold the rep of
    /// `value`.
    fn target(&self, target: &ast::Expr, value: &Expr) -> Result<VarId, Error> {
        let ast::Expr::Name(name) = target else {
            return Err(unsupported(target, "this target"));
        };
        match self.vars.borrow().find(name.id.as_str()) {
            Some((id, rep)) if rep == value.rep() => Ok(id),
            Some(_) => Err(unsupported(target, "a variable with values of two types")),
            // The checker gives the value a type that no rep holds.
            None => Err(unsupported(target, "a variable of this type")),
        }
    }

    /// The value of the variable that `name` reads.
    fn load(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let Some((id, rep)) = self.vars.borrow().find(name.id.as_str()) else {
            return Err(unsupported(name, "a name that the module does not assign"));
        };
        let check = if is_definitely_bound(self.model, name) {
            None
        } else if is_implicit_global(self.model, name.id.as_str()) {
            // Without a value, the read takes the builtin of the name.
            return Err(unsupported(name, "a name that may be a builtin"));
        } else {
            self.vars.borrow_mut().get_mut(id).checked = true;
            Some(self.site(name.range()))
        };
        Ok(Expr::Load {
            var: id,
            rep,
            check,
        })
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
            ast::Expr::Name(name) => self.load(name),
            ast::Expr::UnaryOp(u) => self.unary(u),
            ast::Expr::BinOp(b) => {
                let op = arith_op(b.op).ok_or_else(|| unsupported(expr, "this operator"))?;
                let left = self.int_operand(&b.left)?;
                let right = self.int_operand(&b.right)?;
                Ok(Expr::Arith {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    site: self.site(b.range()),
                })
            }
            ast::Expr::BoolOp(b) => {
                let op = match b.op {
                    ast::BoolOp::And => LogicOp::And,
                    ast::BoolOp::Or => LogicOp::Or,
                };
                let (first, rest) = b
                    .values
                    .split_first()
                    .expect("a bool operator has operands");
                // `a and b and c` gives the value of `(a and b) and c`, and
                // evaluates the same operands.
                rest.iter()
                    .try_fold(self.bool_operand(first)?, |left, right| {
                        Ok(Expr::Logic {
                            op,
                            left: Box::new(left),
                            right: Box::new(self.bool_operand(right)?),
                        })
                    })
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
                    Ok(Expr::Arith {
                        op: ArithOp::Sub,
                        left: Box::new(Expr::Int(0)),
                        right: Box::new(operand),
                        site: self.site(u.range()),
                    })
                }
            },
            ast::UnaryOp::UAdd => self.int_operand(&u.operand),
            ast::UnaryOp::Not => Ok(Expr::Not(Box::new(self.bool_operand(&u.operand)?))),
            ast::UnaryOp::Invert => Err(unsupported(u, "this operator")),
        }
    }

    /// A new site for the instruction at `range`, in the code of the module.
    fn site(&self, range: TextRange) -> Site {
        let mut sites = self.sites.borrow_mut();
        sites.push(traceback::frame(self.path, "<module>", self.source, range));
        Site(u32::try_from(sites.len() - 1).expect("fewer than 2^32 sites"))
    }

    fn bool_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(operand)?;
        match lowered.rep() {
            Rep::Bool => Ok(lowered),
            _ => Err(unsupported(operand, "an operand that is not a bool")),
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
fn truth(lowered: Expr, node: &impl Ranged) -> Result<Expr, Error> {
    match lowered.rep() {
        Rep::Bool => Ok(lowered),
        Rep::Int | Rep::Str => Ok(Expr::Truth(Box::new(lowered))),
        Rep::None => Err(unsupported(node, "the truth of None")),
    }
}

/// The rep of the values of `shape`, and `None` when the compiler has no rep
/// for them.
fn shape_rep(shape: Option<Shape>) -> Option<Rep> {
    match shape? {
        Shape::Int | Shape::IntLit(_) => Some(Rep::Int),
        Shape::Bool | Shape::BoolLit(_) => Some(Rep::Bool),
        Shape::Str | Shape::StrLit(_) => Some(Rep::Str),
        _ => None,
    }
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
