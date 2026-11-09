//! The representations of `int`.

use std::collections::BTreeMap;

use wasm_encoder::{BlockType, ConstExpr, ValType};

use super::str::StrRepr;
use crate::cpython::{
    DIVISION_BY_ZERO, FACTORIAL_OF_A_NEGATIVE, FACTORIAL_TOO_LARGE, INFINITY_TO_INT,
    MODULO_BY_ZERO, NAN_TO_INT, NEGATIVE_K, NEGATIVE_N,
};
use crate::emit::module::{FuncIdx, FuncState, ModuleState};
use crate::emit::runtime::exc::Exceptions;
use crate::ir::{ArithOp, CmpOp};

const MIN: i64 = -(1 << 30);
const MAX: i64 = (1 << 30) - 1;

pub(super) enum IntRepr {
    /// An `i32` that holds 31 bits. A result outside 31 bits raises an
    /// error, so a value never wraps.
    I31 {
        /// `(func (param i32 i32) (result i32))` for `+`, `-`, `*`, `//`
        /// and `%`.
        add: FuncIdx,
        sub: FuncIdx,
        mul: FuncIdx,
        floor_div: FuncIdx,
        modulo: FuncIdx,
        /// `(func (param i64) (result i32))`, which raises an error when
        /// the value does not fit in 31 bits.
        check: FuncIdx,
        /// `(func (param f64) (result i32))` for `int` of a `float`, which
        /// the module gets at the first use.
        convert_f64: Option<FuncIdx>,
        round: Option<FuncIdx>,
        /// `(func (param i64 i32) (result i64))` for a step of `math.gcd`,
        /// which the module gets at the first use. The `gcd` of the least
        /// `int` and 0 does not fit in 31 bits, so it is an `i64` until the
        /// last step.
        gcd: Option<FuncIdx>,
        /// `(func (param i32) (result i32))` for `math.factorial`, which
        /// the module gets at the first use.
        factorial: Option<FuncIdx>,
        /// `(func (param i64 i32) (result i64))` for a step of `math.lcm`,
        /// which the module gets at the first use. An `lcm` that does not
        /// fit in 31 bits stays above `MAX`, but not at its value, so that
        /// it fits in an `i64`.
        lcm: Option<FuncIdx>,
        /// `(func (param i64) (result i32))` for the end of `math.lcm`.
        lcm_end: Option<FuncIdx>,
        /// `(func (param i32 i32) (result i32))` for `math.comb` and
        /// `math.perm`, which the module gets at the first use.
        chooses: BTreeMap<Choice, FuncIdx>,
    },
}

/// The ways of `math.comb` and `math.perm` to choose `k` of `n` things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Choice {
    Comb,
    Perm,
}

impl Choice {
    fn name(self) -> &'static str {
        match self {
            Choice::Comb => "comb",
            Choice::Perm => "perm",
        }
    }
}

impl IntRepr {
    pub fn i31(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> Self {
        let overflow = add_overflow(m, str, exc);
        let check = add_check(m, overflow);
        IntRepr::I31 {
            add: add_arith(m, check, |f| {
                f.ins().i64_add();
            }),
            sub: add_arith(m, check, |f| {
                f.ins().i64_sub();
            }),
            mul: add_arith(m, check, |f| {
                f.ins().i64_mul();
            }),
            floor_div: add_floor_div(m, str, exc, check),
            modulo: add_modulo(m, str, exc),
            check,
            convert_f64: None,
            round: None,
            gcd: None,
            factorial: None,
            lcm: None,
            lcm_end: None,
            chooses: BTreeMap::new(),
        }
    }

    pub fn val_type(&self) -> ValType {
        match self {
            IntRepr::I31 { .. } => ValType::I32,
        }
    }

    /// The initial value of a global that holds an `int`.
    pub fn global_init(&self) -> ConstExpr {
        match self {
            IntRepr::I31 { .. } => ConstExpr::i32_const(0),
        }
    }

    pub fn literal(&self, f: &mut FuncState, value: i32) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().i32_const(value);
            }
        }
    }

    /// Pops a `bool` and pushes it as an `int`.
    pub fn bool_as_int(&self, _f: &mut FuncState) {
        match self {
            // A `bool` is already the `i32` 0 or 1.
            IntRepr::I31 { .. } => {}
        }
    }

    /// Pops an `i32` without sign, the length of a value, and pushes it as
    /// an `int`.
    pub fn len_as_int(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { check, .. } => {
                f.ins().i64_extend_i32_u().call(*check);
            }
        }
    }

    /// Pops an `int` and pushes it as an `i32`.
    pub fn as_i32(&self, _f: &mut FuncState) {
        match self {
            IntRepr::I31 { .. } => {}
        }
    }

    /// Pops an `f64` and pushes it as an `int`, without the fraction. A
    /// value outside 31 bits raises `OverflowError`, which a conversion
    /// may raise in Python, as the conversion of an infinity does.
    pub fn convert_f64(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        match self {
            IntRepr::I31 { convert_f64, .. } => {
                let func = *convert_f64.get_or_insert_with(|| add_convert_f64(m, str, exc));
                f.ins().call(func);
            }
        }
    }

    /// Pops two `int`s and pushes the result of `op`.
    pub fn arith(&self, f: &mut FuncState, op: ArithOp) {
        match self {
            IntRepr::I31 {
                add,
                sub,
                mul,
                floor_div,
                modulo,
                ..
            } => {
                let func = match op {
                    ArithOp::Add => add,
                    ArithOp::Sub => sub,
                    ArithOp::Mul => mul,
                    ArithOp::FloorDiv => floor_div,
                    ArithOp::Mod => modulo,
                };
                f.ins().call(*func);
            }
        }
    }

    /// Pops an `int` and pushes its absolute value, which the `check` of
    /// `-1073741824` stops.
    pub fn abs(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { check, .. } => {
                let v = f.temp(ValType::I64);
                f.ins()
                    .i64_extend_i32_s()
                    .local_set(v)
                    .i64_const(0)
                    .local_get(v)
                    .i64_sub()
                    .local_get(v)
                    .local_get(v)
                    .i64_const(0)
                    .i64_lt_s()
                    .select()
                    .call(*check);
                f.free(v);
            }
        }
    }

    /// Pops an `int` and a number of digits, and pushes the `int` rounded
    /// to those digits.
    pub fn round(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            IntRepr::I31 { round, check, .. } => {
                let func = *round.get_or_insert_with(|| add_round(m, *check));
                f.ins().call(func);
            }
        }
    }

    pub fn gcd_start(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().i64_const(0);
            }
        }
    }

    pub fn gcd_step(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            IntRepr::I31 { gcd, .. } => {
                let func = *gcd.get_or_insert_with(|| add_gcd(m));
                f.ins().call(func);
            }
        }
    }

    pub fn gcd_end(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { check, .. } => {
                f.ins().call(*check);
            }
        }
    }

    pub fn lcm_start(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().i64_const(1);
            }
        }
    }

    pub fn lcm_step(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            IntRepr::I31 { gcd, lcm, .. } => {
                let gcd = *gcd.get_or_insert_with(|| add_gcd(m));
                let func = *lcm.get_or_insert_with(|| add_lcm(m, gcd));
                f.ins().call(func);
            }
        }
    }

    pub fn lcm_end(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        match self {
            IntRepr::I31 { lcm_end, .. } => {
                let func = *lcm_end.get_or_insert_with(|| add_lcm_end(m, str, exc));
                f.ins().call(func);
            }
        }
    }

    pub fn choose(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
        choice: Choice,
    ) {
        match self {
            IntRepr::I31 { chooses, .. } => {
                let func = *chooses
                    .entry(choice)
                    .or_insert_with(|| add_choose(m, str, exc, choice));
                f.ins().call(func);
            }
        }
    }

    pub fn factorial(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        match self {
            IntRepr::I31 { factorial, .. } => {
                let func = *factorial.get_or_insert_with(|| add_factorial(m, str, exc));
                f.ins().call(func);
            }
        }
    }

    /// Pops an `int` that is not negative and pushes its integer square
    /// root. The square root of an `f64` is correctly rounded, so below
    /// `2^52` it never rounds up to the next integer.
    pub fn isqrt(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().f64_convert_i32_s().f64_sqrt().i32_trunc_f64_s();
            }
        }
    }

    /// Pops two `int`s and pushes the `bool` of `op`.
    pub fn compare(&self, f: &mut FuncState, op: CmpOp) {
        match self {
            IntRepr::I31 { .. } => {
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.i32_eq(),
                    CmpOp::Ne => ins.i32_ne(),
                    CmpOp::Lt => ins.i32_lt_s(),
                    CmpOp::Le => ins.i32_le_s(),
                    CmpOp::Gt => ins.i32_gt_s(),
                    CmpOp::Ge => ins.i32_ge_s(),
                };
            }
        }
    }

    /// Pops an `int` and pushes `true` if it is not 0.
    pub fn truth(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().i32_const(0).i32_ne();
            }
        }
    }

    /// Pops an `int` and writes it to the standard output, as one call of
    /// `write` in CPython.
    pub fn write(&self, f: &mut FuncState, str: &StrRepr) {
        match self {
            IntRepr::I31 { .. } => {
                f.ins().i64_extend_i32_s();
                str.decimal(f);
                str.write(f);
            }
        }
    }
}

/// The function of an operator, which runs `op` on the two operands in
/// `i64`, where no result of two 31-bit operands wraps.
fn add_arith(m: &mut ModuleState, check: FuncIdx, op: impl FnOnce(&mut FuncState)) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    f.ins()
        .local_get(a)
        .i64_extend_i32_s()
        .local_get(b)
        .i64_extend_i32_s();
    op(&mut f);
    f.ins().call(check);
    m.add_func(ty, f.finish())
}

/// Raises `ZeroDivisionError` with `message` when the divisor, the second
/// parameter, is 0.
fn check_divisor(
    m: &mut ModuleState,
    f: &mut FuncState,
    str: &mut StrRepr,
    exc: &Exceptions,
    message: &str,
) {
    let divisor = f.param(1);
    f.ins().local_get(divisor).i32_eqz();
    f.if_(BlockType::Empty);
    str.literal(m, f, message);
    exc.raise(f);
    f.end();
}

/// `//` rounds the quotient down, and the division of `i64` rounds it
/// toward 0. The quotient is one less when the signs differ and the
/// division has a remainder. Only `-1073741824 // -1` leaves 31 bits.
fn add_floor_div(
    m: &mut ModuleState,
    str: &mut StrRepr,
    exc: &Exceptions,
    check: FuncIdx,
) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    check_divisor(m, &mut f, str, exc, DIVISION_BY_ZERO);
    f.ins()
        .local_get(a)
        .i64_extend_i32_s()
        .local_get(b)
        .i64_extend_i32_s()
        .i64_div_s()
        .local_get(a)
        .local_get(b)
        .i32_rem_s()
        .i32_const(0)
        .i32_ne()
        .local_get(a)
        .local_get(b)
        .i32_xor()
        .i32_const(0)
        .i32_lt_s()
        .i32_and()
        .i64_extend_i32_u()
        .i64_sub()
        .call(check);
    m.add_func(ty, f.finish())
}

/// `%` takes the sign of the divisor, and the remainder of `i32` takes the
/// sign of the dividend. When the signs differ, the divisor goes back into
/// a remainder that is not 0.
fn add_modulo(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    let r = f.local(ValType::I32);
    check_divisor(m, &mut f, str, exc, MODULO_BY_ZERO);
    f.ins()
        .local_get(a)
        .local_get(b)
        .i32_rem_s()
        .local_tee(r)
        .local_get(b)
        .i32_const(0)
        .local_get(r)
        .i32_const(0)
        .i32_ne()
        .local_get(r)
        .local_get(b)
        .i32_xor()
        .i32_const(0)
        .i32_lt_s()
        .i32_and()
        .select()
        .i32_add();
    m.add_func(ty, f.finish())
}

/// The function of `int` of an `f64`. The fraction goes before the
/// range check, so `-1073741824.9` gives `-1073741824`.
fn add_convert_f64(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let ty = m.func_type(&[ValType::F64], &[ValType::I32]);
    let mut f = FuncState::new(1);
    let x = f.param(0);
    f.ins().local_get(x).local_get(x).f64_ne();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, NAN_TO_INT);
    exc.raise(&mut f);
    f.end();
    f.ins()
        .local_get(x)
        .f64_abs()
        .f64_const(f64::INFINITY.into())
        .f64_eq();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, INFINITY_TO_INT);
    exc.raise(&mut f);
    f.end();
    #[expect(clippy::cast_precision_loss, reason = "31 bits fit in an f64")]
    let (below, above) = ((MIN - 1) as f64, (MAX + 1) as f64);
    f.ins()
        .local_get(x)
        .f64_const(below.into())
        .f64_gt()
        .local_get(x)
        .f64_const(above.into())
        .f64_lt()
        .i32_and();
    f.if_(BlockType::Empty);
    f.ins().local_get(x).i32_trunc_f64_s().return_();
    f.end();
    str.literal(
        m,
        &mut f,
        &format!(
            "OverflowError: cannot convert float to an int of spython, which goes from {MIN} to {MAX}"
        ),
    );
    exc.raise(&mut f);
    m.add_func(ty, f.finish())
}

/// The function of a step of `math.gcd`, which takes the `gcd` so far, a
/// number at least 0, and an `int`, and gives the `gcd` of both by the
/// algorithm of Euclid.
fn add_gcd(m: &mut ModuleState) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64, ValType::I32], &[ValType::I64]);
    let mut f = FuncState::new(2);
    let (a, x) = (f.param(0), f.param(1));
    let (b, t) = (f.local(ValType::I64), f.local(ValType::I64));
    f.ins()
        .local_get(x)
        .i64_extend_i32_s()
        .local_tee(b)
        .i64_const(0)
        .i64_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i64_const(0).local_get(b).i64_sub().local_set(b);
    f.end();
    let done = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(b).i64_eqz();
    f.br_if(done);
    f.ins()
        .local_get(a)
        .local_get(b)
        .i64_rem_u()
        .local_set(t)
        .local_get(b)
        .local_set(a)
        .local_get(t)
        .local_set(b);
    f.br(again);
    f.end();
    f.end();
    f.ins().local_get(a);
    m.add_func(ty, f.finish())
}

/// The function of a step of `math.lcm`, which takes the `lcm` so far, a
/// number from 0 to `MAX + 1`, and an `int`, and gives the `lcm` of both.
/// A result above `MAX + 1` becomes `MAX + 1`, which keeps the product of
/// the next step within an `i64`. Any result above `MAX` stops the
/// program at the end, unless a later 0 makes the `lcm` 0.
fn add_lcm(m: &mut ModuleState, gcd: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64, ValType::I32], &[ValType::I64]);
    let mut f = FuncState::new(2);
    let (a, x) = (f.param(0), f.param(1));
    let (b, r) = (f.local(ValType::I64), f.local(ValType::I64));
    f.ins()
        .local_get(x)
        .i64_extend_i32_s()
        .local_tee(b)
        .i64_const(0)
        .i64_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i64_const(0).local_get(b).i64_sub().local_set(b);
    f.end();
    f.ins()
        .local_get(a)
        .i64_eqz()
        .local_get(b)
        .i64_eqz()
        .i32_or();
    f.if_(BlockType::Empty);
    f.ins().i64_const(0).return_();
    f.end();
    f.ins()
        .local_get(a)
        .local_get(a)
        .local_get(x)
        .call(gcd)
        .i64_div_u()
        .local_get(b)
        .i64_mul()
        .local_set(r);
    f.ins()
        .i64_const(MAX + 1)
        .local_get(r)
        .local_get(r)
        .i64_const(MAX + 1)
        .i64_gt_u()
        .select();
    m.add_func(ty, f.finish())
}

/// The message of a result of `func` that does not fit in an `int`. The
/// message does not give the result, which may not fit in an `i64`.
fn too_large(func: &str) -> String {
    format!("spython: the result of {func} does not fit in an int, which goes from {MIN} to {MAX}")
}

/// The function of `math.comb(n, k)` or `math.perm(n, k)`, which raises as
/// `math_comb_impl` and `math_perm_impl` of CPython do. With `k <= n`, the
/// result is the product of the `k` steps `r * (n - i)`, divided by `i + 1`
/// for `comb`, with `i` from 0. Each step keeps `r` exact. `perm` grows at
/// each step, and so does `comb` with `k` at most `n / 2`, so a step whose
/// `r` does not fit stops the program.
fn add_choose(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions, choice: Choice) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (n, k) = (f.param(0), f.param(1));
    let (i, r) = (f.local(ValType::I32), f.local(ValType::I64));
    for (arg, message) in [(n, NEGATIVE_N), (k, NEGATIVE_K)] {
        f.ins().local_get(arg).i32_const(0).i32_lt_s();
        f.if_(BlockType::Empty);
        str.literal(m, &mut f, message);
        exc.raise(&mut f);
        f.end();
    }
    f.ins().local_get(k).local_get(n).i32_gt_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).return_();
    f.end();
    if choice == Choice::Comb {
        // `comb(n, k)` is `comb(n, n - k)`.
        f.ins()
            .local_get(n)
            .local_get(k)
            .i32_sub()
            .local_get(k)
            .local_get(n)
            .local_get(k)
            .i32_sub()
            .local_get(k)
            .i32_lt_s()
            .select()
            .local_set(k);
    }
    f.ins().i64_const(1).local_set(r);
    let done = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(k).i32_ge_s();
    f.br_if(done);
    f.ins()
        .local_get(r)
        .local_get(n)
        .local_get(i)
        .i32_sub()
        .i64_extend_i32_s()
        .i64_mul()
        .local_set(r);
    f.ins().local_get(i).i32_const(1).i32_add().local_set(i);
    if choice == Choice::Comb {
        f.ins()
            .local_get(r)
            .local_get(i)
            .i64_extend_i32_s()
            .i64_div_u()
            .local_set(r);
    }
    f.ins().local_get(r).i64_const(MAX).i64_gt_s();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, &too_large(choice.name()));
    exc.raise(&mut f);
    f.end();
    f.br(again);
    f.end();
    f.end();
    f.ins().local_get(r).i32_wrap_i64();
    m.add_func(ty, f.finish())
}

/// The function of the end of `math.lcm`, which gives the `lcm` as an
/// `int`, or stops the program when it is above `MAX`.
fn add_lcm_end(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64], &[ValType::I32]);
    let mut f = FuncState::new(1);
    let v = f.param(0);
    f.ins().local_get(v).i64_const(MAX).i64_gt_u();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, &too_large("lcm"));
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(v).i32_wrap_i64();
    m.add_func(ty, f.finish())
}

/// The function of `math.factorial(n)`. `12!` is the greatest factorial
/// that fits in 31 bits. A larger `n` raises the `OverflowError` that
/// CPython raises for an `n` above `2^63 - 1`, with 12 as the limit.
fn add_factorial(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(1);
    let n = f.param(0);
    let r = f.local(ValType::I32);
    f.ins().local_get(n).i32_const(0).i32_lt_s();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, FACTORIAL_OF_A_NEGATIVE);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(n).i32_const(12).i32_gt_s();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, FACTORIAL_TOO_LARGE);
    exc.raise(&mut f);
    f.end();
    f.ins().i32_const(1).local_set(r);
    let done = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(n).i32_const(1).i32_le_s();
    f.br_if(done);
    f.ins()
        .local_get(r)
        .local_get(n)
        .i32_mul()
        .local_set(r)
        .local_get(n)
        .i32_const(1)
        .i32_sub()
        .local_set(n);
    f.br(again);
    f.end();
    f.end();
    f.ins().local_get(r);
    m.add_func(ty, f.finish())
}

/// The function of `round(x, n)` of an `int`. With `n < 0`, the result is
/// the multiple of `p = 10 ** -n` nearest to `x`, with a tie going to the
/// even quotient. `x // p` and `x % p` give the quotient and the rest, and
/// the quotient goes up when the rest is more than half of `p`. With `n <
/// -9`, `p` is more than twice any `int`, and the result is 0.
fn add_round(m: &mut ModuleState, check: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (x, n) = (f.param(0), f.param(1));
    let (p, q, r) = (
        f.local(ValType::I64),
        f.local(ValType::I64),
        f.local(ValType::I64),
    );
    f.ins().local_get(n).i32_const(0).i32_ge_s();
    f.if_(BlockType::Empty);
    f.ins().local_get(x).return_();
    f.end();
    f.ins().local_get(n).i32_const(-9).i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).return_();
    f.end();
    f.ins().i64_const(1).local_set(p);
    let again = f.loop_(BlockType::Empty);
    f.ins()
        .local_get(p)
        .i64_const(10)
        .i64_mul()
        .local_set(p)
        .local_get(n)
        .i32_const(1)
        .i32_add()
        .local_tee(n)
        .i32_const(0)
        .i32_lt_s();
    f.br_if(again);
    f.end();
    f.ins()
        .local_get(x)
        .i64_extend_i32_s()
        .local_get(p)
        .i64_div_s()
        .local_set(q)
        .local_get(x)
        .i64_extend_i32_s()
        .local_get(p)
        .i64_rem_s()
        .local_tee(r)
        .i64_const(0)
        .i64_lt_s();
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(r)
        .local_get(p)
        .i64_add()
        .local_set(r)
        .local_get(q)
        .i64_const(1)
        .i64_sub()
        .local_set(q);
    f.end();
    f.ins()
        .local_get(q)
        .local_get(r)
        .i64_const(1)
        .i64_shl()
        .local_get(p)
        .i64_gt_s()
        .local_get(r)
        .i64_const(1)
        .i64_shl()
        .local_get(p)
        .i64_eq()
        .local_get(q)
        .i32_wrap_i64()
        .i32_const(1)
        .i32_and()
        .i32_and()
        .i32_or()
        .i64_extend_i32_u()
        .i64_add()
        .local_get(p)
        .i64_mul()
        .call(check);
    m.add_func(ty, f.finish())
}

/// `(func (param i64) (result i32))`, which raises an error when the
/// value does not fit in 31 bits.
fn add_check(m: &mut ModuleState, overflow: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64], &[ValType::I32]);
    let mut f = FuncState::new(1);
    let v = f.param(0);
    f.ins()
        .local_get(v)
        .i64_const(-MIN)
        .i64_add()
        .i64_const(MAX - MIN + 1)
        .i64_lt_u();
    f.if_(BlockType::Empty);
    f.ins().local_get(v).i32_wrap_i64().return_();
    f.end();
    f.ins().local_get(v).call(overflow).unreachable();
    m.add_func(ty, f.finish())
}

/// `(func (param i64))`, which raises the error of a result outside 31
/// bits, with the result and the range.
fn add_overflow(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64], &[]);
    let mut f = FuncState::new(1);
    let v = f.param(0);
    str.literal(m, &mut f, "spython: the result ");
    f.ins().local_get(v);
    str.decimal(&mut f);
    str.concat(&mut f);
    str.literal(
        m,
        &mut f,
        &format!(" does not fit in an int, which goes from {MIN} to {MAX}"),
    );
    str.concat(&mut f);
    exc.raise(&mut f);
    m.add_func(ty, f.finish())
}
