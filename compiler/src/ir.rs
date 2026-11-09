//! The program after lowering. Every Python decision is already made here,
//! and the emitter only translates.

pub(crate) struct Program {
    pub body: Vec<Stmt>,
    /// The variables of the module, by their [`VarId`].
    pub vars: Vec<Var>,
    /// The functions of the module, by their [`FuncId`].
    pub funcs: Vec<Func>,
    /// The names that an import binds, by their [`ImportId`].
    pub imports: Vec<Import>,
    /// The frame of each place that can raise, by its [`Site`].
    pub sites: Vec<SiteFrame>,
}

pub(crate) struct SiteFrame {
    /// What the traceback writes for the frame.
    pub text: String,
    /// The id of the line of the frame, which two sites share when they are
    /// on the same line of the same function.
    pub line: u32,
}

/// A place in the program that can raise an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Site(u32);

/// The ids of the IR, each the index of what it names in its list.
macro_rules! id {
    ($($name:ident, $what:literal;)*) => {
        $(
            impl $name {
                pub fn new(index: usize) -> Self {
                    $name(u32::try_from(index).expect(concat!("fewer than 2^32 ", $what)))
                }

                pub fn index(self) -> usize {
                    self.0 as usize
                }
            }
        )*
    };
}

id! {
    Site, "sites";
    VarId, "variables";
    FuncId, "functions";
    ImportId, "imports";
}

/// A variable of the module, or of a function, by its place in the list
/// of the variables of its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VarId(u32);

/// A variable of the module or of the function that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Global(VarId),
    Local(VarId),
}

pub(crate) struct Var {
    pub rep: Rep,
    /// `true` when the program asks whether the variable has a value, as in
    /// a read that may come before the first assignment.
    pub flagged: bool,
    /// `true` when the program asks which globals got a value first. Only a
    /// flagged variable of the module is ordered.
    pub ordered: bool,
}

/// A function of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FuncId(u32);

/// A name of the module that an import binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImportId(u32);

pub(crate) struct Import {
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
}

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
    /// The import of each name, from which on a read finds it.
    Import(Vec<ImportId>),
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
    Float(f64),
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
    /// An `int` read as a `float`, as an operand of an operator of `float`.
    IntAsFloat(Box<Expr>),
    /// `+`, `-` or `*` on two `float`s, which raises nothing.
    FloatArith {
        op: FloatOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `-` on a `float`, which gives `-0.0` for `0.0`.
    FloatNeg(Box<Expr>),
    /// `abs` of an `int`. The result of the least `int` does not fit, and
    /// stops the program at the site.
    IntAbs {
        operand: Box<Expr>,
        site: Site,
    },
    /// `round` of an `int` to `digits` digits, which rounds to a multiple
    /// of a power of 10 when `digits` is negative. A result outside the
    /// range of an `int` stops the program at the site.
    IntRound {
        operand: Box<Expr>,
        digits: Box<Expr>,
        site: Site,
    },
    /// `int` of a `float`, which drops the fraction. `nan`, an infinity and
    /// a value outside the range of an `int` raise at the site.
    FloatAsInt {
        operand: Box<Expr>,
        site: Site,
    },
    /// `abs` of a `float`, which gives `0.0` for `-0.0`.
    FloatAbs(Box<Expr>),
    /// A `float` rounded to the nearest whole value, with a tie going to
    /// the even one, as `round` does.
    FloatRound(Box<Expr>),
    /// `value`, once the import of `import` has run. A read of a name of
    /// an import before the import raises `NameError`.
    AfterImport {
        import: ImportId,
        check: Check,
        value: Box<Expr>,
    },
    /// A function of `math` of a `float`. A result that CPython rejects
    /// raises at the site.
    MathCall {
        func: MathFunc,
        operand: Box<Expr>,
        site: Site,
    },
    /// A function of `math` of two `float`s. A result that CPython rejects
    /// raises at the site.
    Math2Call {
        func: MathFunc2,
        x: Box<Expr>,
        y: Box<Expr>,
        site: Site,
    },
    /// `math.fma(x, y, z)` of three `float`s. A nan from values that are
    /// not nan, and an infinity from finite values, raise at the site.
    Fma {
        x: Box<Expr>,
        y: Box<Expr>,
        z: Box<Expr>,
        site: Site,
    },
    /// A function of `math` of two `float`s that never raises, with `x`
    /// the first argument and `y` the second.
    Float2Call {
        func: FloatFunc2,
        x: Box<Expr>,
        y: Box<Expr>,
    },
    /// `math.ldexp(x, i)` of a `float` and an `int`. A result too large
    /// for a `float` raises `OverflowError` at the site.
    Ldexp {
        x: Box<Expr>,
        i: Box<Expr>,
        site: Site,
    },
    /// `math.log(x, base)` of two `float`s, which raises at the site as
    /// `math.log` does for each of them, or `ZeroDivisionError` for a base
    /// of 1.
    LogBase {
        operand: Box<Expr>,
        base: Box<Expr>,
        site: Site,
    },
    /// `math.gcd` of any number of `int`s. A result that does not fit in an
    /// `int` stops the program at the site.
    IntGcd {
        args: Vec<Expr>,
        site: Site,
    },
    /// `math.lcm` of any number of `int`s. A result that does not fit in an
    /// `int` stops the program at the site.
    IntLcm {
        args: Vec<Expr>,
        site: Site,
    },
    /// `math.comb(n, k)` or `math.perm(n, k)` of two `int`s, by `choice`.
    /// A negative `int` raises `ValueError` at the site, and a result that
    /// does not fit in an `int` stops the program there.
    IntChoose {
        choice: Choice,
        n: Box<Expr>,
        k: Box<Expr>,
        site: Site,
    },
    /// `math.factorial` of an `int`. A negative `int` raises `ValueError`
    /// at the site, and an `int` above 12 raises `OverflowError` there.
    IntFactorial {
        operand: Box<Expr>,
        site: Site,
    },
    /// `math.isqrt` of an `int`. A negative `int` raises `ValueError` at
    /// the site.
    IntIsqrt {
        operand: Box<Expr>,
        site: Site,
    },
    /// `math.fmod(x, y)` of two `float`s, which raises `ValueError` at the
    /// site for a `y` of 0 or an infinite `x`.
    Fmod {
        x: Box<Expr>,
        y: Box<Expr>,
        site: Site,
    },
    /// `math.isclose(a, b)` of two `float`s, as a `bool`, with the
    /// tolerances that the call gives in the order of the call. A negative
    /// tolerance raises `ValueError` at the site.
    IsClose {
        a: Box<Expr>,
        b: Box<Expr>,
        tolerances: Vec<(Tolerance, Expr)>,
        site: Site,
    },
    /// Whether a `float` is of `class`, as a `bool`.
    FloatIs {
        class: FloatClass,
        operand: Box<Expr>,
    },
    /// The square root of a `float`. A value below 0 raises `ValueError`
    /// at the site.
    FloatSqrt {
        operand: Box<Expr>,
        site: Site,
    },
    /// The greatest whole `float` not above the `float`, as `math.floor`.
    FloatFloor(Box<Expr>),
    /// The least whole `float` not below the `float`, as `math.ceil`.
    FloatCeil(Box<Expr>),
    /// `round` of a `float` to `digits` digits. A rounded value too large
    /// for a `float` raises `OverflowError` at the site.
    FloatRoundDigits {
        operand: Box<Expr>,
        digits: Box<Expr>,
        site: Site,
    },
    /// `/`, `//` or `%` on two `float`s. A divisor of 0 raises
    /// `ZeroDivisionError` at the site.
    Div {
        op: DivOp,
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
    /// `**` on two `float`s, which raises at the site the errors of CPython
    /// and the error of a result that would be a `complex`.
    FloatPow {
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
    /// Two `str`s, one after the other. The site is where the program runs
    /// out of memory for the result.
    Concat {
        left: Box<Expr>,
        right: Box<Expr>,
        site: Site,
    },
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
    /// The truth of an `int`, a `float` or a `str`, as a `bool`. A number
    /// is true when it is not 0, and a `str` when it is not empty. `nan` is
    /// true, and `-0.0` is false.
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
    /// A chain of comparisons of `int`s, of `float`s or of `str`s, as in
    /// `a < b <= c`.
    /// Each operand after the first comes with the operator on its left.
    Compare(Box<Expr>, Vec<(CmpOp, Expr)>),
    /// `min` or `max` of two or more values of the same rep. As in CPython,
    /// a value replaces the result only when it is less, for `min`, or
    /// greater, for `max`, so `max(nan, 1.0)` is `nan` and `max(-0.0, 0.0)`
    /// is `-0.0`.
    MinMax {
        op: MinMaxOp,
        args: Vec<Expr>,
    },
    /// `print` with the default `sep` and `end`. A write that fails raises
    /// `OSError` at the site.
    Print {
        args: Vec<Expr>,
        site: Site,
    },
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
    /// The message when no candidate has a value.
    pub message: String,
    /// The message with a suggestion is the first text, the suggestion and
    /// the second text.
    pub suggested: (String, String),
    /// A `self` whose value has an attribute of the name, which CPython
    /// suggests before any candidate when it has a value, with the text of
    /// that suggestion.
    pub self_attr: Option<(Binding, String)>,
    /// The candidates with a value count against the limit of CPython, when
    /// the frame may have that many.
    pub limit: Option<Limit>,
    /// The candidates that may be the suggestion, by their distance to the
    /// name, from the least. The first group with a candidate that has a
    /// value gives the suggestion.
    pub groups: Vec<Group>,
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
    Import(ImportId),
}

pub(crate) struct Limit {
    /// The number of candidates with a value at which CPython gives up.
    pub max: u32,
    /// The candidates that the frame always has.
    pub always: u32,
    /// How many times each global with a value counts. The frame of the
    /// module lists its globals twice.
    pub globals: u32,
    /// The locals of the frame that count when they have a value.
    pub locals: Vec<VarId>,
}

/// A function of `math` of one `float` that has a builtin of the same name
/// and raises as `math_1` of CPython does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MathFunc {
    Exp,
    Exp2,
    Expm1,
    Log,
    Log2,
    Log10,
    Log1p,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Cbrt,
    Erf,
    Erfc,
    Gamma,
    Lgamma,
    Ulp,
}

impl MathFunc {
    const ALL: [(MathFunc, &'static str); 25] = [
        (MathFunc::Exp, "exp"),
        (MathFunc::Exp2, "exp2"),
        (MathFunc::Expm1, "expm1"),
        (MathFunc::Log, "log"),
        (MathFunc::Log2, "log2"),
        (MathFunc::Log10, "log10"),
        (MathFunc::Log1p, "log1p"),
        (MathFunc::Sin, "sin"),
        (MathFunc::Cos, "cos"),
        (MathFunc::Tan, "tan"),
        (MathFunc::Asin, "asin"),
        (MathFunc::Acos, "acos"),
        (MathFunc::Atan, "atan"),
        (MathFunc::Sinh, "sinh"),
        (MathFunc::Cosh, "cosh"),
        (MathFunc::Tanh, "tanh"),
        (MathFunc::Asinh, "asinh"),
        (MathFunc::Acosh, "acosh"),
        (MathFunc::Atanh, "atanh"),
        (MathFunc::Cbrt, "cbrt"),
        (MathFunc::Erf, "erf"),
        (MathFunc::Erfc, "erfc"),
        (MathFunc::Gamma, "gamma"),
        (MathFunc::Lgamma, "lgamma"),
        (MathFunc::Ulp, "ulp"),
    ];

    /// The function that `math` names `name`.
    pub fn of(name: &str) -> Option<MathFunc> {
        Self::ALL
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(func, _)| *func)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(func, _)| *func == self)
            .map(|(_, name)| *name)
            .expect("every function is in ALL")
    }

    /// Returns `true` if an infinite result of a finite value raises
    /// `OverflowError`, `false` when it raises `ValueError`, as in CPython.
    pub fn can_overflow(self) -> bool {
        matches!(
            self,
            MathFunc::Exp
                | MathFunc::Exp2
                | MathFunc::Expm1
                | MathFunc::Sinh
                | MathFunc::Cosh
                | MathFunc::Gamma
                | MathFunc::Lgamma
        )
    }
}

/// A function of `math` of two `float`s that has a builtin and raises as
/// `math_2` of CPython does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MathFunc2 {
    Remainder,
    Pow,
}

impl MathFunc2 {
    pub fn of(name: &str) -> Option<MathFunc2> {
        match name {
            "remainder" => Some(MathFunc2::Remainder),
            "pow" => Some(MathFunc2::Pow),
            _ => None,
        }
    }

    pub fn builtin(self) -> &'static str {
        match self {
            MathFunc2::Remainder => "f64_remainder",
            MathFunc2::Pow => "f64_math_pow",
        }
    }
}

/// A function of `math` of two `float`s that never raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatFunc2 {
    Atan2,
    Hypot,
    Nextafter,
    Copysign,
}

impl FloatFunc2 {
    const ALL: [(FloatFunc2, &'static str); 4] = [
        (FloatFunc2::Atan2, "atan2"),
        (FloatFunc2::Hypot, "hypot"),
        (FloatFunc2::Nextafter, "nextafter"),
        (FloatFunc2::Copysign, "copysign"),
    ];

    pub fn of(name: &str) -> Option<FloatFunc2> {
        Self::ALL
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(func, _)| *func)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(func, _)| *func == self)
            .map(|(_, name)| *name)
            .expect("every function is in ALL")
    }
}

/// The ways of `math.comb` and `math.perm` to choose `k` of `n` things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Choice {
    Comb,
    Perm,
}

impl Choice {
    pub fn name(self) -> &'static str {
        match self {
            Choice::Comb => "comb",
            Choice::Perm => "perm",
        }
    }
}

/// A tolerance of `math.isclose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tolerance {
    /// `rel_tol`, `1e-09` by default.
    Rel,
    /// `abs_tol`, `0.0` by default.
    Abs,
}

/// The values that `math.isnan`, `math.isinf` and `math.isfinite` accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatClass {
    Nan,
    Inf,
    /// Neither `nan` nor an infinity.
    Finite,
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
pub(crate) enum FloatOp {
    Add,
    Sub,
    Mul,
}

/// An operator of `float` that divides, which decides the message of the
/// `ZeroDivisionError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DivOp {
    /// `/`, where `ints` is `true` when both operands were `int`s or
    /// `bool`s.
    True {
        ints: bool,
    },
    Floor,
    Mod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinMaxOp {
    Min,
    Max,
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
    Float,
    Bool,
    Str,
    None,
}

impl Expr {
    pub fn rep(&self) -> Rep {
        match self {
            Expr::Str(_) | Expr::Concat { .. } | Expr::Repeat { .. } | Expr::Index { .. } => {
                Rep::Str
            }
            Expr::Int(_)
            | Expr::BoolAsInt(_)
            | Expr::Arith { .. }
            | Expr::IntAbs { .. }
            | Expr::IntGcd { .. }
            | Expr::IntLcm { .. }
            | Expr::IntChoose { .. }
            | Expr::IntFactorial { .. }
            | Expr::IntIsqrt { .. }
            | Expr::IntRound { .. }
            | Expr::FloatAsInt { .. }
            | Expr::Len { .. } => Rep::Int,
            Expr::Float(_)
            | Expr::IntAsFloat(_)
            | Expr::FloatArith { .. }
            | Expr::FloatNeg(_)
            | Expr::FloatAbs(_)
            | Expr::FloatRound(_)
            | Expr::FloatFloor(_)
            | Expr::FloatSqrt { .. }
            | Expr::MathCall { .. }
            | Expr::Float2Call { .. }
            | Expr::Math2Call { .. }
            | Expr::Fma { .. }
            | Expr::Ldexp { .. }
            | Expr::Fmod { .. }
            | Expr::LogBase { .. }
            | Expr::FloatCeil(_)
            | Expr::FloatRoundDigits { .. }
            | Expr::Div { .. }
            | Expr::FloatPow { .. } => Rep::Float,
            Expr::Bool(_)
            | Expr::FloatIs { .. }
            | Expr::IsClose { .. }
            | Expr::Truth(_)
            | Expr::Not(_)
            | Expr::Compare(..) => Rep::Bool,
            Expr::Load { rep, .. } | Expr::Call { rep, .. } => *rep,
            Expr::Logic { left, .. } => left.rep(),
            Expr::AfterImport { value, .. } => value.rep(),
            Expr::MinMax { args, .. } => args.first().expect("min or max has values").rep(),
            Expr::Print { .. } => Rep::None,
        }
    }
}
