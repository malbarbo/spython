//! The program after lowering. Every Python decision is already made here,
//! and the emitter only translates.

use crate::cpython::Class;

pub(crate) struct Program {
    pub body: Vec<Stmt>,
    /// The code that runs the doctests after the body, for a program that
    /// tests its file. An exception of the body ends such a program with
    /// the text of [`crate::cpython::RAISED_BEFORE_TESTS`] after the
    /// traceback.
    pub tests: Option<Vec<Stmt>>,
    /// The variables of the module, by their [`VarId`].
    pub vars: Vec<Var>,
    /// The functions of the module, by their [`FuncId`].
    pub funcs: Vec<Func>,
    /// The names that an import binds, by their [`ImportId`].
    pub imports: Vec<Import>,
    /// The classes of the module, by their [`ClassId`].
    pub classes: Vec<UserClass>,
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

impl Site {
    /// The site of the code of a function without a frame.
    pub const NONE: Site = Site(u32::MAX);
}

id! {
    Site, "sites";
    VarId, "variables";
    FuncId, "functions";
    ImportId, "imports";
    ClassId, "classes";
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

/// A class of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ClassId(u32);

pub(crate) struct UserClass {
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
    /// The rep of each field of an object of the class, in order.
    pub fields: Vec<Rep>,
    /// `true` when the program assigns the fields of an object.
    pub mutable: bool,
    /// The objects that the class has from the start, as the members of an
    /// `Enum` are, with a literal for each field.
    pub members: Vec<Vec<Expr>>,
    /// The function that gives the `str` of an object, when the program
    /// writes one.
    pub str: Option<FuncId>,
    /// `true` when the `repr` of an object writes `...` for the object
    /// inside its own `repr`, as the `repr` of a dataclass does.
    pub repr_guard: bool,
}

pub(crate) struct Func {
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
    /// `false` for a function of the prelude, which has no frame in a
    /// traceback, as a builtin of CPython has none. Its code has no site,
    /// and an exception that it raises has the site of the call.
    pub frame: bool,
    /// The first variables are the parameters, in order.
    pub params: u32,
    pub vars: Vec<Var>,
    pub ret: Rep,
    pub body: Vec<Stmt>,
}

pub(crate) enum Stmt {
    /// An expression whose value the program drops.
    Expr(Expr),
    /// The value goes to each variable, from the first one. Each variable
    /// holds the values of the rep of the value, and gets the value in its
    /// own rep.
    Assign {
        targets: Vec<Place>,
        value: Expr,
    },
    /// The value of the global `from` goes to `to`, which holds the values
    /// of its rep, when `from` has one. Without it, `to` stays as it was.
    Copy {
        from: VarId,
        to: Place,
    },
    /// The `def` of the function, from which on a call finds it.
    Def(FuncId),
    /// The import of each name, from which on a read finds it.
    Import(Vec<ImportId>),
    /// The `class` statement, from which on a read finds the class.
    Class(ClassId),
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
    /// Runs an example of a docstring. What the body writes to the standard
    /// output goes to a buffer, and an exception of the body ends the body
    /// and nothing else, unless its class is [`Class::Limit`]. After the
    /// example, [`Expr::Captured`] gives what the body wrote, and
    /// [`Expr::CapturedException`] the exception.
    Example(Vec<Stmt>),
    /// Ends the program, with the exit code 0 when the `int` is 0, and 1
    /// otherwise.
    Exit(Expr),
    /// Sets the field at `field` of an object of `class` to the value,
    /// which has the rep of the field. The value runs before the object, as
    /// in CPython.
    SetField {
        object: Expr,
        class: ClassId,
        field: u32,
        value: Expr,
    },
    /// Sets the code point at the index of a `str` that the prelude still
    /// builds. The index is in range.
    StrSet {
        builder: Expr,
        index: Expr,
        value: Expr,
    },
    /// Raises an exception of the class at the site, with the `str` as the
    /// text of its argument, or without an argument.
    Raise {
        class: Class,
        arg: Option<Expr>,
        site: Site,
    },
}

pub(crate) enum Expr {
    Str(String),
    /// An `int` that fits in 31 bits.
    Int(i32),
    Float(f64),
    Bool(bool),
    None,
    /// A value of a rep that a value of `rep` also holds, as a `str` is a
    /// `str` or `None`.
    Widen {
        value: Box<Expr>,
        rep: Rep,
    },
    /// A value of a union that the checker narrows to `rep`, one of the
    /// types of the union.
    Narrow {
        value: Box<Expr>,
        rep: Rep,
    },
    /// Whether a value is `None`, as a `bool`.
    IsNone(Box<Expr>),
    /// Whether two objects, each of a class or `None`, are the same object,
    /// as a `bool`.
    Is(Box<Expr>, Box<Expr>),
    /// The value of a variable. Only a read with a check may come before
    /// the first assignment.
    Load {
        place: Place,
        rep: Rep,
        check: Option<Check>,
    },
    /// A call with one argument for each parameter, of the same rep. The
    /// arguments come in the order of the call, and `order` has the
    /// argument of each parameter, or `None` when each parameter takes the
    /// argument in its place. Only a call with a check may come before
    /// the `def`. A call of a function without a frame that raises nothing
    /// has no site.
    Call {
        func: FuncId,
        args: Vec<Expr>,
        order: Option<Vec<u32>>,
        rep: Rep,
        site: Option<Site>,
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
    /// The object that `value` gives. A value of an object may be null,
    /// because a call may assign `None` to a field after the test that
    /// narrows the field, and a null raises `AttributeError` with `text` at
    /// the site, as an attribute of `None` does in CPython.
    NotNone {
        value: Box<Expr>,
        text: String,
        site: Site,
    },
    /// The value of `value`, a union, as a value of `rep`, the type that
    /// the checker gives to the read of a field after a test. A call may
    /// assign the field after the test, and a value of another type raises
    /// `TypeError` with `text` at the site.
    Expect {
        value: Box<Expr>,
        rep: Rep,
        text: String,
        site: Site,
    },
    /// The field at `field` of an object of `class`, which has `rep`.
    Field {
        object: Box<Expr>,
        class: ClassId,
        field: u32,
        rep: Rep,
    },
    /// A new object of `class`, with a value of the rep of each field.
    New {
        class: ClassId,
        fields: Vec<Expr>,
    },
    /// Whether the `repr` of the object runs, as a `bool`.
    InRepr(Box<Expr>),
    /// `value`, which runs with `object` among the objects whose `repr`
    /// runs. An exception leaves the object there, and an example of a
    /// docstring starts with no object there, as the `finally` of CPython
    /// leaves none.
    DuringRepr {
        object: Box<Expr>,
        value: Box<Expr>,
    },
    /// The object at `member` among the members of `class`.
    Member {
        class: ClassId,
        member: u32,
    },
    /// `value`, once the name of `binding` has a value. A read of the name
    /// before raises the error of `check`, as a read of a name of an
    /// import before the import does.
    After {
        binding: Binding,
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
    /// `math.hypot` of `float`s, at most [`HYPOT_MAX`] of them. It never
    /// raises.
    Hypot(Vec<Expr>),
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
    /// `base ** exp` of two `int`s, with an `exp` that is not negative.
    /// The site is where a result outside 31 bits stops the program.
    IntPow {
        base: Box<Expr>,
        exp: Box<Expr>,
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
    /// The `str` of an `int`, a `float` or a `bool`. The site is where the
    /// program runs out of memory for the result.
    StrOf {
        operand: Box<Expr>,
        site: Site,
    },
    /// The `str`s of an f-string, one after the other. The program
    /// evaluates every part before it joins them, and the site is where it
    /// runs out of memory for the result.
    Join {
        parts: Vec<Expr>,
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
    /// What the body of the last [`Stmt::Example`] wrote to the standard
    /// output, as a `str`.
    Captured,
    /// The last line of the traceback of the exception that ended the body
    /// of the last [`Stmt::Example`], with its `\n`, as a `str`. An example
    /// that raised nothing gives an empty `str`.
    CapturedException,
    /// Writes a `str` to the standard error, and flushes it.
    WriteErr(Box<Expr>),
    /// Stops the program at the site with the error of spython of a `str`,
    /// for what CPython does and spython cannot.
    Stop {
        text: Box<Expr>,
        site: Site,
    },
    /// A new `str` of `len` code points, each of them 0, which the prelude
    /// builds with [`Stmt::StrSet`]. The site is where the program runs out
    /// of memory for it.
    StrNew {
        len: Box<Expr>,
        site: Site,
    },
    /// The code point of a `str` at an index in range, as an `int`.
    CodePoint {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    /// The code point at an `int` index of the mapping `case` of a code
    /// point, as an `int`, or -1 when the mapping has fewer code points.
    CaseAt {
        case: Case,
        operand: Box<Expr>,
        index: Box<Expr>,
    },
    /// The value of a code point as a decimal digit, as an `int`, or -1
    /// when the code point is not a decimal digit.
    DecimalValue(Box<Expr>),
    /// The `float` of a `str` of ASCII in the form of a `float` of Python
    /// without underscores.
    ParseFloat(Box<Expr>),
    /// Whether a code point is in `set`, as a `bool`.
    CodePointIs {
        set: CodePointSet,
        operand: Box<Expr>,
    },
    /// The code point of a `str` at an `int` index, as a `str`. An index
    /// out of range raises `IndexError` at the site.
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
        site: Site,
    },
    /// The code points of a `str` from the `int` `start` up to the `int`
    /// `stop`, as a `str`. The slice goes from the first code point without
    /// a `start`, and to the last one without a `stop`. The site is where
    /// the program runs out of memory for the result.
    Slice {
        value: Box<Expr>,
        start: Option<Box<Expr>>,
        stop: Option<Box<Expr>>,
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
    /// `then if cond else else_`, with `then` and `else_` of the same rep.
    /// Only the value that `cond` picks runs.
    IfElse {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Box<Expr>,
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
        args: Vec<PrintArg>,
        site: Site,
    },
}

/// An argument of `print`. As in CPython, `str` converts the value when
/// `print` writes it, after every argument has its value, and the call of
/// `str` has the site of the `print`.
pub(crate) struct PrintArg {
    pub(crate) value: Expr,
    pub(crate) str: Option<FuncId>,
}

/// What a read or a call raises at the site when the name has no value.
pub(crate) struct Check {
    pub site: Site,
    pub error: Undefined,
}

pub(crate) enum Undefined {
    /// The text of the `UnboundLocalError`.
    Local(String),
    Global(Box<NameError>),
}

/// A `NameError`, whose message suggests a name that the frame has at the
/// raise, as CPython does. The candidates are the locals of the frame, the
/// globals in the order of their first binding, and the builtins.
pub(crate) struct NameError {
    /// The text of the error when no candidate has a value.
    pub text: String,
    /// The text with a suggestion is the first text, the suggestion and the
    /// second text.
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
    Class(ClassId),
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

/// The most arguments of a `math.hypot` that the compiler takes. The module
/// passes them to the builtin in the scratch area of the host.
pub(crate) const HYPOT_MAX: usize = 504;

/// A function of `math` of two `float`s that never raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatFunc2 {
    Atan2,
    Nextafter,
    Copysign,
}

impl FloatFunc2 {
    const ALL: [(FloatFunc2, &'static str); 3] = [
        (FloatFunc2::Atan2, "atan2"),
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

/// A set of code points of Unicode, which a builtin tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodePointSet {
    /// The code points that `str.isprintable` accepts.
    Printable,
    /// The code points with the property `Cased`.
    Cased,
    /// The code points with the property `Case_Ignorable`.
    CaseIgnorable,
    /// The code points with the property `Uppercase`.
    Uppercase,
    /// The code points with the property `Lowercase`.
    Lowercase,
    /// The code points that `str.isalpha` accepts.
    Alpha,
    /// The code points that `str.isdecimal` accepts.
    Decimal,
    /// The code points that `str.isdigit` accepts.
    Digit,
    /// The code points that `str.isnumeric` accepts.
    Numeric,
    /// The code points with the property `XID_Start`.
    XidStart,
    /// The code points with the property `XID_Continue`.
    XidContinue,
}

impl CodePointSet {
    /// Each set, with its function in the stub of the intrinsics and its
    /// builtin.
    const NAMES: [(CodePointSet, &str, &str); 11] = [
        (CodePointSet::Printable, "is_printable", "str_is_printable"),
        (CodePointSet::Cased, "is_cased", "str_is_cased"),
        (
            CodePointSet::CaseIgnorable,
            "is_case_ignorable",
            "str_is_case_ignorable",
        ),
        (CodePointSet::Uppercase, "is_uppercase", "str_is_uppercase"),
        (CodePointSet::Lowercase, "is_lowercase", "str_is_lowercase"),
        (CodePointSet::Alpha, "is_alpha", "str_is_alpha"),
        (CodePointSet::Decimal, "is_decimal", "str_is_decimal"),
        (CodePointSet::Digit, "is_digit", "str_is_digit"),
        (CodePointSet::Numeric, "is_numeric", "str_is_numeric"),
        (CodePointSet::XidStart, "is_xid_start", "str_is_xid_start"),
        (
            CodePointSet::XidContinue,
            "is_xid_continue",
            "str_is_xid_continue",
        ),
    ];

    /// The set that the function `name` of the stub of the intrinsics tests.
    pub fn of(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(_, stub, _)| *stub == name)
            .map(|(set, ..)| *set)
    }

    pub fn builtin(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(set, ..)| *set == self)
            .map(|(.., builtin)| *builtin)
            .expect("a builtin for each set")
    }
}

/// A mapping of the case of a code point to code points, as Unicode
/// gives it with the special casings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    Upper,
    Lower,
    Title,
    Fold,
}

impl Case {
    /// Each mapping, with its function in the stub of the intrinsics and
    /// its builtin.
    const NAMES: [(Case, &str, &str); 4] = [
        (Case::Upper, "upper_at", "str_upper_at"),
        (Case::Lower, "lower_at", "str_lower_at"),
        (Case::Title, "title_at", "str_title_at"),
        (Case::Fold, "fold_at", "str_fold_at"),
    ];

    /// The mapping that the function `name` of the stub of the intrinsics
    /// gives.
    pub fn of(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(_, stub, _)| *stub == name)
            .map(|(case, ..)| *case)
    }

    pub fn builtin(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(case, ..)| *case == self)
            .map(|(.., builtin)| *builtin)
            .expect("a builtin for each mapping")
    }
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
    /// A `str` or `None`.
    StrOrNone,
    /// A value of any of the other reps, as a union of types holds.
    Uniform,
    /// An object of the class.
    Object(ClassId),
    /// An object of the class or `None`.
    ObjectOrNone(ClassId),
}

impl Expr {
    pub fn rep(&self) -> Rep {
        match self {
            Expr::Str(_)
            | Expr::Concat { .. }
            | Expr::StrOf { .. }
            | Expr::Join { .. }
            | Expr::Repeat { .. }
            | Expr::StrNew { .. }
            | Expr::Slice { .. }
            | Expr::Captured
            | Expr::CapturedException
            | Expr::Index { .. } => Rep::Str,
            Expr::Int(_)
            | Expr::BoolAsInt(_)
            | Expr::Arith { .. }
            | Expr::IntAbs { .. }
            | Expr::IntGcd { .. }
            | Expr::IntLcm { .. }
            | Expr::IntChoose { .. }
            | Expr::IntPow { .. }
            | Expr::IntRound { .. }
            | Expr::FloatAsInt { .. }
            | Expr::CodePoint { .. }
            | Expr::CaseAt { .. }
            | Expr::DecimalValue(_)
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
            | Expr::Hypot(_)
            | Expr::Math2Call { .. }
            | Expr::Fma { .. }
            | Expr::Ldexp { .. }
            | Expr::Fmod { .. }
            | Expr::LogBase { .. }
            | Expr::FloatCeil(_)
            | Expr::FloatRoundDigits { .. }
            | Expr::Div { .. }
            | Expr::FloatPow { .. }
            | Expr::ParseFloat(_) => Rep::Float,
            Expr::Bool(_)
            | Expr::IsNone(_)
            | Expr::Is(..)
            | Expr::CodePointIs { .. }
            | Expr::IsClose { .. }
            | Expr::Truth(_)
            | Expr::Not(_)
            | Expr::Compare(..)
            | Expr::InRepr(_) => Rep::Bool,
            Expr::None => Rep::None,
            Expr::Load { rep, .. }
            | Expr::Call { rep, .. }
            | Expr::Widen { rep, .. }
            | Expr::Narrow { rep, .. }
            | Expr::Expect { rep, .. }
            | Expr::Field { rep, .. } => *rep,
            Expr::Member { class, .. } | Expr::New { class, .. } => Rep::Object(*class),
            Expr::Logic { left, .. } => left.rep(),
            Expr::IfElse { then, .. } => then.rep(),
            Expr::After { value, .. }
            | Expr::DuringRepr { value, .. }
            | Expr::NotNone { value, .. } => value.rep(),
            Expr::MinMax { args, .. } => args.first().expect("min or max has values").rep(),
            Expr::Print { .. } | Expr::WriteErr(_) | Expr::Stop { .. } => Rep::None,
        }
    }
}
