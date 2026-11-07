//! The representations of `int`.

use wasm_encoder::{BlockType, ConstExpr, ValType};

use super::exc::Exceptions;
use super::str::StrRepr;
use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};
use crate::ir::{ArithOp, CmpOp};

const MIN: i64 = -(1 << 30);
const MAX: i64 = (1 << 30) - 1;

pub(super) enum IntRepr {
    /// An `i32` in a local and an `i31ref` in a uniform slot. A result
    /// outside 31 bits stops the program, so a value never wraps.
    I31 {
        /// `(func (param i32 i32) (result i32))` for `+`, `-`, `*`, `//`
        /// and `%`.
        add: u32,
        sub: u32,
        mul: u32,
        floor_div: u32,
        modulo: u32,
        /// `(func (param i64))`, which writes the number in decimal to the
        /// standard output.
        write: u32,
    },
}

impl IntRepr {
    pub fn i31(m: &mut ModuleState, host: &Host, str: &mut StrRepr, exc: &Exceptions) -> Self {
        let write = add_write(m, host);
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
            write,
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

    /// Pops an `int` and writes it to the standard output.
    pub fn write(&self, f: &mut FuncState) {
        match self {
            IntRepr::I31 { write, .. } => {
                f.ins().i64_extend_i32_s().call(*write);
            }
        }
    }
}

/// The function of an operator, which runs `op` on the two operands in
/// `i64`, where no result of two 31-bit operands wraps.
fn add_arith(m: &mut ModuleState, check: u32, op: impl FnOnce(&mut FuncState)) -> u32 {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let mut f = FuncState::new(2);
    f.ins()
        .local_get(0)
        .i64_extend_i32_s()
        .local_get(1)
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
    f.ins().local_get(1).i32_eqz().if_(BlockType::Empty);
    str.literal(m, f, message);
    exc.raise(f);
    f.ins().end();
}

/// `//` rounds the quotient down, and the division of `i64` rounds it
/// toward 0. The quotient is one less when the signs differ and the
/// division has a remainder. Only `-1073741824 // -1` leaves 31 bits.
fn add_floor_div(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions, check: u32) -> u32 {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let (a, b) = (0, 1);
    let mut f = FuncState::new(2);
    check_divisor(
        m,
        &mut f,
        str,
        exc,
        "ZeroDivisionError: integer division or modulo by zero",
    );
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
fn add_modulo(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> u32 {
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
    let (a, b) = (0, 1);
    let mut f = FuncState::new(2);
    let r = f.local(ValType::I32);
    check_divisor(
        m,
        &mut f,
        str,
        exc,
        "ZeroDivisionError: integer modulo by zero",
    );
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

/// `(func (param i64) (result i32))`, which stops the program when the
/// value does not fit in 31 bits.
fn add_check(m: &mut ModuleState, overflow: u32) -> u32 {
    let ty = m.func_type(&[ValType::I64], &[ValType::I32]);
    let v = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .local_get(v)
        .i64_const(-MIN)
        .i64_add()
        .i64_const(MAX - MIN + 1)
        .i64_lt_u()
        .if_(BlockType::Empty)
        .local_get(v)
        .i32_wrap_i64()
        .return_()
        .end()
        .local_get(v)
        .call(overflow)
        .unreachable();
    m.add_func(ty, f.finish())
}

/// `(func (param i64))`, which raises the error of a result outside 31
/// bits, with the result and the range.
fn add_overflow(m: &mut ModuleState, str: &mut StrRepr, exc: &Exceptions) -> u32 {
    let ty = m.func_type(&[ValType::I64], &[]);
    let v = 0;
    let mut f = FuncState::new(1);
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

/// `(func (param i64))`, which writes the number in decimal.
fn add_write(m: &mut ModuleState, host: &Host) -> u32 {
    let ty = m.func_type(&[ValType::I64], &[]);
    let digits = m.reserve(ty);
    let v = 0;
    // The digits of a number that is not negative, the first ones by a
    // call to itself.
    let mut f = FuncState::new(1);
    f.ins()
        .local_get(v)
        .i64_const(10)
        .i64_ge_u()
        .if_(BlockType::Empty)
        .local_get(v)
        .i64_const(10)
        .i64_div_u()
        .call(digits)
        .end()
        .local_get(v)
        .i64_const(10)
        .i64_rem_u()
        .i32_wrap_i64()
        .i32_const('0' as i32)
        .i32_add()
        .call(host.write_char);
    m.define(digits, f.finish());

    let mut f = FuncState::new(1);
    f.ins()
        .local_get(v)
        .i64_const(0)
        .i64_lt_s()
        .if_(BlockType::Empty)
        .i32_const('-' as i32)
        .call(host.write_char)
        .i64_const(0)
        .local_get(v)
        .i64_sub()
        .local_set(v)
        .end()
        .local_get(v)
        .call(digits);
    m.add_func(ty, f.finish())
}
