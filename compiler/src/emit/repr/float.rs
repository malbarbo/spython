//! The representations of `float`.

use std::collections::BTreeMap;

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, ValType};

use super::str::StrRepr;
use crate::cpython::{
    FLOAT_FLOOR_DIVISION_BY_ZERO, FLOAT_MODULO_BY_ZERO, FLOAT_TRUE_DIVISION_BY_ZERO,
    INT_TRUE_DIVISION_BY_ZERO, MATH_DOMAIN_ERROR, MATH_RANGE_ERROR, RESULT_OUT_OF_RANGE,
    ROUNDED_TOO_LARGE, ZERO_TO_A_NEGATIVE_POWER,
};
use crate::emit::module::{FuncIdx, FuncState, ModuleState};
use crate::emit::runtime::exc::Exceptions;
use crate::emit::runtime::host::Host;
use crate::ir::{CmpOp, DivOp, FloatClass, FloatOp, MathFunc};

pub(super) enum FloatRepr {
    /// An `f64`.
    F64 {
        /// `(func (param f64))`, which writes the `repr` of the `float`. A
        /// module that writes no `float` goes without it, and without the
        /// builtin that it calls.
        write: Option<FuncIdx>,
        /// `(func (param f64 f64) (result f64))` for each operator that
        /// divides, which the module gets at the first use.
        divs: BTreeMap<DivOp, FuncIdx>,
        /// `(func (param f64 f64) (result f64))` for `**`, which the module
        /// gets at the first use.
        pow: Option<FuncIdx>,
        /// `(func (param f64 i32) (result f64))` for `round(x, n)`, which
        /// the module gets at the first use.
        round_digits: Option<FuncIdx>,
        /// `(func (param f64) (result f64))` for each function of `math`
        /// from the libm, which the module gets at the first use.
        maths: BTreeMap<MathFunc, FuncIdx>,
        /// `(func (param f64 f64) (result f64))` for `math.log(x, base)`,
        /// which the module gets at the first use.
        log_base: Option<FuncIdx>,
        write_char: FuncIdx,
        end_write: FuncIdx,
        scratch: i32,
    },
}

impl FloatRepr {
    pub fn f64(host: &Host) -> Self {
        FloatRepr::F64 {
            write: None,
            divs: BTreeMap::new(),
            pow: None,
            round_digits: None,
            maths: BTreeMap::new(),
            log_base: None,
            write_char: host.write_char,
            end_write: host.end_write,
            scratch: host.scratch,
        }
    }

    pub fn val_type(&self) -> ValType {
        match self {
            FloatRepr::F64 { .. } => ValType::F64,
        }
    }

    /// The initial value of a global that holds a `float`.
    pub fn global_init(&self) -> ConstExpr {
        match self {
            FloatRepr::F64 { .. } => ConstExpr::f64_const(0.0.into()),
        }
    }

    pub fn literal(&self, f: &mut FuncState, value: f64) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_const(value.into());
            }
        }
    }

    /// Pops an `i32` and pushes it as a `float`. Every `i32` fits in the 53
    /// bits of the mantissa.
    pub fn convert_i32(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_convert_i32_s();
            }
        }
    }

    /// Pops two `float`s and pushes the result of `op`.
    pub fn arith(&self, f: &mut FuncState, op: FloatOp) {
        match self {
            FloatRepr::F64 { .. } => {
                let mut ins = f.ins();
                match op {
                    FloatOp::Add => ins.f64_add(),
                    FloatOp::Sub => ins.f64_sub(),
                    FloatOp::Mul => ins.f64_mul(),
                };
            }
        }
    }

    pub fn div(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
        op: DivOp,
    ) {
        match self {
            FloatRepr::F64 { divs, .. } => {
                let div = *divs.entry(op).or_insert_with(|| add_div(m, str, exc, op));
                f.ins().call(div);
            }
        }
    }

    pub fn pow(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        match self {
            FloatRepr::F64 { pow, .. } => {
                let pow = *pow.get_or_insert_with(|| add_pow(m, str, exc));
                f.ins().call(pow);
            }
        }
    }

    /// Pops a `float` and a number of digits, and pushes the `float`
    /// rounded to those digits, or raises `OverflowError` when the rounded
    /// value is too large for a `float`.
    pub fn round_digits(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        match self {
            FloatRepr::F64 { round_digits, .. } => {
                let func = *round_digits.get_or_insert_with(|| add_round_digits(m, str, exc));
                f.ins().call(func);
            }
        }
    }

    /// Pops a `float` and pushes `true` when it is not 0. `nan` is not 0.
    pub fn truth(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_const(0.0.into()).f64_ne();
            }
        }
    }

    /// Pops two `float`s and pushes the `bool` of `op`. A comparison with
    /// `nan` is false, except `!=`, as in Python.
    pub fn compare(&self, f: &mut FuncState, op: CmpOp) {
        match self {
            FloatRepr::F64 { .. } => {
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.f64_eq(),
                    CmpOp::Ne => ins.f64_ne(),
                    CmpOp::Lt => ins.f64_lt(),
                    CmpOp::Le => ins.f64_le(),
                    CmpOp::Gt => ins.f64_gt(),
                    CmpOp::Ge => ins.f64_ge(),
                };
            }
        }
    }

    pub fn neg(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_neg();
            }
        }
    }

    pub fn abs(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_abs();
            }
        }
    }

    pub fn copysign(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_copysign();
            }
        }
    }

    /// Pops a `float` and pushes the nearest whole value, with a tie going
    /// to the even one.
    pub fn round(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_nearest();
            }
        }
    }

    /// Pops a `float` and pushes `func` of it, or raises the error of
    /// CPython.
    pub fn math_call(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
        func: MathFunc,
    ) {
        match self {
            FloatRepr::F64 { maths, .. } => {
                let index = *maths
                    .entry(func)
                    .or_insert_with(|| add_math(m, str, exc, func));
                f.ins().call(index);
            }
        }
    }

    /// Pops `y` and `x` and pushes `math.atan2(y, x)`, which never raises.
    pub fn atan2(&self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                let builtin = m.builtin("f64_atan2");
                f.ins().call(builtin);
            }
        }
    }

    /// Pops `x` and `y` and pushes `math.hypot(x, y)`, which never raises.
    pub fn hypot(&self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                let builtin = m.builtin("f64_hypot");
                f.ins().call(builtin);
            }
        }
    }

    /// Pops `x` and a base and pushes `math.log(x, base)`, or raises the
    /// error of CPython.
    pub fn log_base(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        str: &mut StrRepr,
        exc: &Exceptions,
    ) {
        let log = match self {
            FloatRepr::F64 { maths, .. } => *maths
                .entry(MathFunc::Log)
                .or_insert_with(|| add_math(m, str, exc, MathFunc::Log)),
        };
        match self {
            FloatRepr::F64 { log_base, .. } => {
                let index = *log_base.get_or_insert_with(|| add_log_base(m, str, exc, log));
                f.ins().call(index);
            }
        }
    }

    /// Pops a `float` and pushes the `bool` of whether it is of `class`.
    pub fn is(&self, f: &mut FuncState, class: FloatClass) {
        match self {
            FloatRepr::F64 { .. } => {
                let infinity = f64::INFINITY.into();
                match class {
                    // `nan` is the one value that differs from itself.
                    FloatClass::Nan => {
                        let x = f.temp(ValType::F64);
                        f.ins().local_tee(x).local_get(x).f64_ne();
                        f.free(x);
                    }
                    FloatClass::Inf => {
                        f.ins().f64_abs().f64_const(infinity).f64_eq();
                    }
                    // A comparison with `nan` is false.
                    FloatClass::Finite => {
                        f.ins().f64_abs().f64_const(infinity).f64_lt();
                    }
                }
            }
        }
    }

    pub fn sqrt(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_sqrt();
            }
        }
    }

    /// Pops a `float` and pushes the greatest whole value not above it.
    pub fn floor(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_floor();
            }
        }
    }

    /// Pops a `float` and pushes the least whole value not below it.
    pub fn ceil(&self, f: &mut FuncState) {
        match self {
            FloatRepr::F64 { .. } => {
                f.ins().f64_ceil();
            }
        }
    }

    /// Pops a `float` and writes its `repr` to the standard output, as one
    /// write.
    pub fn write(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            FloatRepr::F64 {
                write,
                write_char,
                end_write,
                scratch,
                ..
            } => {
                let write =
                    *write.get_or_insert_with(|| add_write(m, *write_char, *end_write, *scratch));
                f.ins().call(write);
            }
        }
    }
}

/// The function of `op`, which raises `ZeroDivisionError` with the message
/// of CPython when the divisor is 0 or -0. `//` and `%` call the port of
/// CPython in the builtins.
fn add_div(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions, op: DivOp) -> FuncIdx {
    let (message, builtin) = match op {
        DivOp::True { ints: true } => (INT_TRUE_DIVISION_BY_ZERO, None),
        DivOp::True { ints: false } => (FLOAT_TRUE_DIVISION_BY_ZERO, None),
        DivOp::Floor => (FLOAT_FLOOR_DIVISION_BY_ZERO, Some("f64_floor_div")),
        DivOp::Mod => (FLOAT_MODULO_BY_ZERO, Some("f64_mod")),
    };
    let builtin = builtin.map(|name| m.builtin(name));
    let ty = m.func_type(&[ValType::F64, ValType::F64], &[ValType::F64]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    f.ins().local_get(b).f64_const(0.0.into()).f64_eq();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, message);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(a).local_get(b);
    match builtin {
        Some(builtin) => f.ins().call(builtin),
        None => f.ins().f64_div(),
    };
    m.add_func(ty, f.finish())
}

/// The message of a negative base with an exponent that is not whole,
/// where CPython gives a `complex`. The `float-exponent` lint rejects the
/// programs that could get here.
const COMPLEX_POWER: &str = "spython: a negative number to a fractional power gives a complex number, \
     which spython does not have";

/// The function of `**`, which calls `f64_pow`. `f64_pow` gives a result
/// that is not finite for finite operands only where CPython raises, or
/// where it gives a `complex`, so the function looks at the operands only
/// then.
fn add_pow(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let builtin = m.builtin("f64_pow");
    let ty = m.func_type(&[ValType::F64, ValType::F64], &[ValType::F64]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    let r = f.local(ValType::F64);
    // `x - x` is 0 for a finite `x`, and nan otherwise.
    f.ins()
        .local_get(a)
        .local_get(b)
        .call(builtin)
        .local_tee(r)
        .local_get(r)
        .f64_sub()
        .f64_const(0.0.into())
        .f64_ne()
        .local_get(a)
        .local_get(a)
        .f64_sub()
        .local_get(b)
        .local_get(b)
        .f64_sub()
        .f64_add()
        .f64_const(0.0.into())
        .f64_eq()
        .i32_and();
    f.if_(BlockType::Empty);
    // An infinite result from a base of 0 has a negative exponent.
    f.ins().local_get(a).f64_const(0.0.into()).f64_eq();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, ZERO_TO_A_NEGATIVE_POWER);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(r).local_get(r).f64_ne();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, COMPLEX_POWER);
    exc.raise(&mut f);
    f.end();
    str.literal(m, &mut f, RESULT_OUT_OF_RANGE);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(r);
    m.add_func(ty, f.finish())
}

/// The function of `func`, which calls its builtin and raises as
/// `math_1` in `Modules/mathmodule.c` of CPython does. A `nan` from a value
/// that is not `nan` raises `ValueError`, and an infinity from a finite
/// value raises `OverflowError` or `ValueError`, by `can_overflow`.
fn add_math(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions, func: MathFunc) -> FuncIdx {
    let builtin = m.builtin(&format!("f64_{}", func.name()));
    let ty = m.func_type(&[ValType::F64], &[ValType::F64]);
    let mut f = FuncState::new(1);
    let x = f.param(0);
    let r = f.local(ValType::F64);
    f.ins()
        .local_get(x)
        .call(builtin)
        .local_tee(r)
        .local_get(r)
        .f64_ne()
        .local_get(x)
        .local_get(x)
        .f64_eq()
        .i32_and();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, MATH_DOMAIN_ERROR);
    exc.raise(&mut f);
    f.end();
    // `r` is not nan here, so `r - r` is not 0 for an infinity alone, and
    // `x - x` is 0 for a finite `x` alone.
    f.ins()
        .local_get(r)
        .local_get(r)
        .f64_sub()
        .f64_const(0.0.into())
        .f64_ne()
        .local_get(x)
        .local_get(x)
        .f64_sub()
        .f64_const(0.0.into())
        .f64_eq()
        .i32_and();
    f.if_(BlockType::Empty);
    let message = if func.can_overflow() {
        MATH_RANGE_ERROR
    } else {
        MATH_DOMAIN_ERROR
    };
    str.literal(m, &mut f, message);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(r);
    m.add_func(ty, f.finish())
}

/// The function of `math.log(x, base)`, which is `log(x) / log(base)`, with
/// `log` the function of `math.log`. A base of 1 raises
/// `ZeroDivisionError`.
fn add_log_base(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions, log: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::F64, ValType::F64], &[ValType::F64]);
    let mut f = FuncState::new(2);
    let (x, base) = (f.param(0), f.param(1));
    let den = f.local(ValType::F64);
    f.ins()
        .local_get(x)
        .call(log)
        .local_get(base)
        .call(log)
        .local_tee(den)
        .f64_const(0.0.into())
        .f64_eq();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, FLOAT_TRUE_DIVISION_BY_ZERO);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(den).f64_div();
    m.add_func(ty, f.finish())
}

/// `f64_round` gives an infinity for a finite value whose rounded value is
/// too large for a `float`, where CPython raises `OverflowError`.
fn add_round_digits(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> FuncIdx {
    let builtin = m.builtin("f64_round");
    let ty = m.func_type(&[ValType::F64, ValType::I32], &[ValType::F64]);
    let mut f = FuncState::new(2);
    let (x, n) = (f.param(0), f.param(1));
    let r = f.local(ValType::F64);
    // `x - x` is 0 for a finite `x`, and nan otherwise.
    f.ins()
        .local_get(x)
        .local_get(n)
        .call(builtin)
        .local_tee(r)
        .local_get(r)
        .f64_sub()
        .f64_const(0.0.into())
        .f64_ne()
        .local_get(x)
        .local_get(x)
        .f64_sub()
        .f64_const(0.0.into())
        .f64_eq()
        .i32_and();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, ROUNDED_TOO_LARGE);
    exc.raise(&mut f);
    f.end();
    f.ins().local_get(r);
    m.add_func(ty, f.finish())
}

/// `f64_repr` writes the ASCII of the `repr` to the scratch area of the
/// host, and the function passes each byte to `write_char`.
fn add_write(
    m: &mut ModuleState,
    write_char: FuncIdx,
    end_write: FuncIdx,
    scratch: i32,
) -> FuncIdx {
    let repr = m.builtin("f64_repr");
    let ty = m.func_type(&[ValType::F64], &[]);
    let mut f = FuncState::new(1);
    let x = f.param(0);
    let i = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    f.ins()
        .local_get(x)
        .i32_const(scratch)
        .call(repr)
        .local_set(len);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(len).i32_ge_u();
    f.br_if(exit);
    f.ins()
        .local_get(i)
        .i32_load8_u(MemArg {
            offset: u64::try_from(scratch).expect("a positive address"),
            align: 0,
            target: MemArgTarget::Memory(0),
            ordering: None,
        })
        .call(write_char)
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i);
    f.br(again);
    f.end();
    f.end();
    f.ins().call(end_write);
    m.add_func(ty, f.finish())
}
