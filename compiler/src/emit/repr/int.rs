//! The representations of `int`.

use wasm_encoder::{BlockType, ConstExpr, ValType};

use super::str::StrRepr;
use crate::cpython::{DIVISION_BY_ZERO, MODULO_BY_ZERO};
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
    },
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
