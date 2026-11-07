//! The program after lowering. Every Python decision is already made here,
//! and the emitter only translates.

pub(crate) struct Program {
    pub body: Vec<Stmt>,
    /// The text of the frame of each place that can raise, by its [`Site`].
    pub sites: Vec<String>,
}

/// A place in the program that can raise an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Site(pub u32);

pub(crate) enum Stmt {
    /// An expression whose value the program drops.
    Expr(Expr),
}

pub(crate) enum Expr {
    Str(String),
    /// An `int` that fits in 31 bits.
    Int(i32),
    Bool(bool),
    /// A `bool` read as an `int`, as an operand of an operator of `int`.
    BoolAsInt(Box<Expr>),
    /// An operator on two `int`s.
    Arith {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
    /// A chain of comparisons of `int`s, as in `a < b <= c`. Each operand
    /// after the first comes with the operator on its left.
    Compare(Box<Expr>, Vec<(CmpOp, Expr)>),
    /// `print` with the default `sep` and `end`.
    Print(Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArithOp {
    Add,
    Sub,
    Mul,
    FloorDiv,
    Mod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// The shape of a value, which knows nothing of wasm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rep {
    Int,
    Bool,
    Str,
    None,
}

impl Expr {
    pub fn rep(&self) -> Rep {
        match self {
            Expr::Str(_) => Rep::Str,
            Expr::Int(_) | Expr::BoolAsInt(_) | Expr::Arith { .. } => Rep::Int,
            Expr::Bool(_) | Expr::Compare(..) => Rep::Bool,
            Expr::Print(_) => Rep::None,
        }
    }
}
