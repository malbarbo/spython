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
    /// `true` when the program asks whether the variable has a value, as a
    /// read that may come before the first assignment does.
    pub flagged: bool,
    /// `true` when the program asks which globals got a value first. Only a
    /// flagged variable of the module is ordered.
    pub ordered: bool,
}

/// A function of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FuncId(pub u32);

pub(crate) struct Func {
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
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
    /// The value of a variable. Only a read with a check may come before
    /// the first assignment.
    Load {
        place: Place,
        rep: Rep,
        check: Option<Check>,
    },
    /// A call with one argument for each parameter, of the same rep. Only a
    /// call with a check may come before the `def`.
    Call {
        func: FuncId,
        args: Vec<Expr>,
        rep: Rep,
        site: Site,
        check: Option<Check>,
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
    /// Two `str`s, one after the other.
    Concat(Box<Expr>, Box<Expr>),
    /// A `str` repeated an `int` number of times, where either operand may
    /// come first. The result raises `MemoryError` at the site when it is
    /// too long.
    Repeat {
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
    /// The code point of a `str` at an `int` index, as a `str`. An index
    /// out of range raises `IndexError` at the site.
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
        site: Site,
    },
    /// `len` of a `str`. A length outside 31 bits stops the program at the
    /// site.
    Len {
        operand: Box<Expr>,
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
    /// A chain of comparisons of `int`s or of `str`s, as in `a < b <= c`.
    /// Each operand after the first comes with the operator on its left.
    Compare(Box<Expr>, Vec<(CmpOp, Expr)>),
    /// `print` with the default `sep` and `end`.
    Print(Vec<Expr>),
}

/// What a read or a call raises at the site when the name has no value.
pub(crate) struct Check {
    pub site: Site,
    pub error: Undefined,
}

pub(crate) enum Undefined {
    /// The message of the `UnboundLocalError`.
    Local(String),
    Global(Box<NameError>),
}

/// A `NameError`, whose message suggests a name that the frame has at the
/// raise, as CPython does. The candidates are the locals of the frame, the
/// globals in the order of their first binding, and the builtins.
pub(crate) struct NameError {
    pub name: String,
    /// A `self` whose value has an attribute of the name, which CPython
    /// suggests before any candidate when it has a value.
    pub self_attr: Option<Binding>,
    /// The candidates with a value count against the limit of CPython, when
    /// the frame may have that many.
    pub limit: Option<Limit>,
    /// The candidates that may be the suggestion, by their distance to the
    /// name, from the least. The first group with a candidate that has a
    /// value gives the suggestion.
    pub groups: Vec<Group>,
    /// `true` when the name is a module of the standard library, which
    /// CPython asks about.
    pub module: bool,
}

/// The candidates at one distance from the name.
pub(crate) struct Group {
    /// The first local with a value wins, in the order of the frame.
    pub locals: Vec<(VarId, String)>,
    /// Otherwise the global that got a value first wins.
    pub globals: Vec<(Binding, String)>,
    /// Otherwise a name that the frame always has wins.
    pub always: Option<String>,
}

/// A name that may have a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    Local(VarId),
    Global(VarId),
    Func(FuncId),
}

pub(crate) struct Limit {
    /// The candidates that the frame always has.
    pub always: u32,
    /// How many times each global with a value counts. The frame of the
    /// module lists its globals twice.
    pub globals: u32,
    /// The locals of the frame that count when they have a value.
    pub locals: Vec<VarId>,
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
            Expr::Str(_) | Expr::Concat(..) | Expr::Repeat { .. } | Expr::Index { .. } => Rep::Str,
            Expr::Int(_) | Expr::BoolAsInt(_) | Expr::Arith { .. } | Expr::Len { .. } => Rep::Int,
            Expr::Bool(_) | Expr::Truth(_) | Expr::Not(_) | Expr::Compare(..) => Rep::Bool,
            Expr::Load { rep, .. } | Expr::Call { rep, .. } => *rep,
            Expr::Logic { left, .. } => left.rep(),
            Expr::Print(_) => Rep::None,
        }
    }
}
