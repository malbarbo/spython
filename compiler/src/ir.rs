//! The program after lowering. Every Python decision is already made here,
//! and the emitter only translates.

pub(crate) struct Program {
    pub body: Vec<Stmt>,
    /// The variables of the module, by their [`VarId`].
    pub vars: Vec<Var>,
    /// The functions of the module, by their [`FuncId`].
    pub funcs: Vec<Func>,
    /// The frame of each place that can raise, by its [`Site`].
    pub sites: Vec<SiteFrame>,
}

pub(crate) struct SiteFrame {
    /// What the traceback writes for the frame.
    pub text: String,
    /// The line of the frame in its function. Two sites have the same line
    /// when they are on the same line of the same function.
    pub line: u32,
}

/// A place in the program that can raise an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Site(pub u32);

/// A variable of the module, or of a function, by its place in the list
/// of the variables of its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VarId(pub u32);

/// A variable of the module or of the function that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Global(VarId),
    Local(VarId),
}

pub(crate) struct Var {
    pub rep: Rep,
    /// The message of the `NameError` or of the `UnboundLocalError` that a
    /// read before the first assignment raises.
    pub undefined: String,
    /// `true` when some read of the variable may come before its first
    /// assignment.
    pub checked: bool,
}

/// A function of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FuncId(pub u32);

pub(crate) struct Func {
    /// The message of the `NameError` that a call before the `def` raises.
    pub undefined: String,
    /// The first variables are the parameters, in order.
    pub params: u32,
    pub vars: Vec<Var>,
    pub ret: Rep,
    pub body: Vec<Stmt>,
}

pub(crate) enum Stmt {
    /// An expression whose value the program drops.
    Expr(Expr),
    /// The value goes to each variable, from the first one.
    Assign {
        targets: Vec<Place>,
        value: Expr,
    },
    /// The `def` of the function, from which on a call finds it.
    Def(FuncId),
    /// The value has the rep of the result of the function. A bare
    /// `return` and `return None` have no value.
    Return(Option<Expr>),
    /// The condition is a `bool`.
    If {
        cond: Expr,
        then: Vec<Stmt>,
        else_: Vec<Stmt>,
    },
    /// The condition is a `bool`, and `else_` runs when the condition is
    /// false, not after a `break`.
    While {
        cond: Expr,
        body: Vec<Stmt>,
        else_: Vec<Stmt>,
    },
    Break,
    Continue,
}

pub(crate) enum Expr {
    Str(String),
    /// An `int` that fits in 31 bits.
    Int(i32),
    Bool(bool),
    /// The value of a variable. The read raises `NameError` at the site when
    /// the variable has no value yet, and only a read with a site may come
    /// before the first assignment.
    Load {
        place: Place,
        rep: Rep,
        check: Option<Site>,
    },
    /// A call with one argument for each parameter, of the same rep. The
    /// call raises `NameError` at `check` when the `def` has not run yet.
    Call {
        func: FuncId,
        args: Vec<Expr>,
        rep: Rep,
        site: Site,
        check: Option<Site>,
    },
    /// A `bool` read as an `int`, as an operand of an operator of `int`.
    BoolAsInt(Box<Expr>),
    /// An operator on two `int`s.
    Arith {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
    /// The truth of an `int` or a `str`, as a `bool`. An `int` is true
    /// when it is not 0, and a `str` when it is not empty.
    Truth(Box<Expr>),
    /// `not` on a `bool`, which may be the truth of another value.
    Not(Box<Expr>),
    /// `and` or `or` on two values of the same rep, which gives one of
    /// them. The right operand runs only when the truth of the left one
    /// does not decide the result.
    Logic {
        op: LogicOp,
        left: Box<Expr>,
        right: Box<Expr>,
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
pub(crate) enum LogicOp {
    And,
    Or,
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
            Expr::Bool(_) | Expr::Truth(_) | Expr::Not(_) | Expr::Compare(..) => Rep::Bool,
            Expr::Load { rep, .. } | Expr::Call { rep, .. } => *rep,
            Expr::Logic { left, .. } => left.rep(),
            Expr::Print(_) => Rep::None,
        }
    }
}
