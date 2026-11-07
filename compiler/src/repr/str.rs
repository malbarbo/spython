//! The representations of `str`.

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, HeapType, RefType, StorageType, ValType};

use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};

/// The most code points in a `str`, the longest array that an `i32`
/// length gives.
const MAX_LEN: i64 = i32::MAX as i64;

pub(super) enum StrRepr {
    /// One code point per element of an `(array (mut i32))`, so `s[i]` is
    /// O(1). The array is mutable only while a function builds it.
    Ucs4 {
        array: u32,
        /// `(func (param (ref $Str)))`, which writes the `str` to the
        /// standard output.
        write: u32,
        /// `(func (param i64) (result (ref $Str)))`, the decimal digits.
        decimal: u32,
        /// `(func (param (ref $Str) (ref $Str)) (result (ref $Str)))`.
        concat: u32,
        /// `(func (param (ref $Str) i32) (result (ref null $Str)))`, which
        /// repeats the `str`, and gives null when the result is too long.
        repeat: u32,
        /// `(func (param (ref $Str) (ref $Str)) (result i32))`, which gives
        /// -1, 0 or 1 as the first `str` comes before, with or after the
        /// second one.
        compare: u32,
        /// The global that holds each literal, by its text.
        literals: HashMap<String, u32>,
    },
}

impl StrRepr {
    pub fn ucs4(m: &mut ModuleState, host: &Host) -> Self {
        let array = m.array_type(StorageType::Val(ValType::I32), true);
        let write = add_write(m, host, array);
        StrRepr::Ucs4 {
            array,
            write,
            decimal: add_decimal(m, array),
            concat: add_concat(m, array),
            repeat: add_repeat(m, array),
            compare: add_compare(m, array),
            literals: HashMap::new(),
        }
    }

    /// Pushes the literal `s`.
    pub fn literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        let global = self.literal_global(m, s);
        f.ins().global_get(global).ref_as_non_null();
    }

    /// The global that holds the literal `s`. Each literal is built once,
    /// by `_start` before the program runs.
    pub fn literal_global(&mut self, m: &mut ModuleState, s: &str) -> u32 {
        match self {
            StrRepr::Ucs4 {
                array, literals, ..
            } => {
                let array = *array;
                *literals.entry(s.to_owned()).or_insert_with(|| {
                    let bytes: Vec<u8> = s.chars().flat_map(|c| (c as u32).to_le_bytes()).collect();
                    let data = m.passive_data(bytes);
                    let global = m.global(
                        ref_to(array, true),
                        &ConstExpr::ref_null(HeapType::Concrete(array)),
                    );
                    let len =
                        i32::try_from(s.chars().count()).expect("a literal shorter than 2^31");
                    wasm_encoder::InstructionSink::new(&mut m.init)
                        .i32_const(0)
                        .i32_const(len)
                        .array_new_data(array, data)
                        .global_set(global);
                    global
                })
            }
        }
    }

    /// The index of the array type of a `str`.
    pub fn type_index(&self) -> u32 {
        match self {
            StrRepr::Ucs4 { array, .. } => *array,
        }
    }

    pub fn val_type(&self, nullable: bool) -> ValType {
        match self {
            StrRepr::Ucs4 { array, .. } => ref_to(*array, nullable),
        }
    }

    /// Pops an `i64` and pushes its decimal digits.
    pub fn decimal(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { decimal, .. } => {
                f.ins().call(*decimal);
            }
        }
    }

    /// Pops a `str` and pushes `true` if it is not empty.
    pub fn truth(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { .. } => {
                f.ins().array_len().i32_const(0).i32_ne();
            }
        }
    }

    /// Pops two `str`s and pushes the second one after the first.
    pub fn concat(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { concat, .. } => {
                f.ins().call(*concat);
            }
        }
    }

    /// Pops a `str` and an `i32`, and pushes the `str` repeated that many
    /// times, or null when the result has more than [`MAX_LEN`] code
    /// points.
    pub fn repeat(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { repeat, .. } => {
                f.ins().call(*repeat);
            }
        }
    }

    /// Pops two `str`s and pushes -1, 0 or 1 as the first one comes
    /// before, with or after the second one, by the order of their code
    /// points.
    pub fn compare(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { compare, .. } => {
                f.ins().call(*compare);
            }
        }
    }

    pub fn write(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { write, .. } => {
                f.ins().call(*write);
            }
        }
    }
}

fn ref_to(index: u32, nullable: bool) -> ValType {
    ValType::Ref(RefType {
        nullable,
        heap_type: HeapType::Concrete(index),
    })
}

fn add_write(m: &mut ModuleState, host: &Host, array: u32) -> u32 {
    let ty = m.func_type(&[ref_to(array, false)], &[]);
    let s = 0;
    let mut f = FuncState::new(1);
    let i = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    f.ins()
        .local_get(s)
        .array_len()
        .local_set(len)
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(i)
        .local_get(len)
        .i32_ge_u()
        .br_if(1)
        .local_get(s)
        .local_get(i)
        .array_get(array)
        .call(host.write_char)
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i)
        .br(0)
        .end()
        .end();
    m.add_func(ty, f.finish())
}

fn add_decimal(m: &mut ModuleState, array: u32) -> u32 {
    let ty = m.func_type(&[ValType::I64], &[ref_to(array, false)]);
    let v = 0;
    let mut f = FuncState::new(1);
    let negative = f.local(ValType::I32);
    let n = f.local(ValType::I64);
    let len = f.local(ValType::I32);
    let rest = f.local(ValType::I64);
    let s = f.local(ref_to(array, true));
    f.ins()
        .local_get(v)
        .i64_const(0)
        .i64_lt_s()
        .local_set(negative)
        .local_get(v)
        .local_set(n)
        .local_get(negative)
        .if_(BlockType::Empty)
        .i64_const(0)
        .local_get(v)
        .i64_sub()
        .local_set(n)
        .end()
        // The digits, one more for each division by 10 that leaves a rest.
        .local_get(negative)
        .i32_const(1)
        .i32_add()
        .local_set(len)
        .local_get(n)
        .i64_const(10)
        .i64_div_u()
        .local_set(rest)
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(rest)
        .i64_eqz()
        .br_if(1)
        .local_get(len)
        .i32_const(1)
        .i32_add()
        .local_set(len)
        .local_get(rest)
        .i64_const(10)
        .i64_div_u()
        .local_set(rest)
        .br(0)
        .end()
        .end()
        .local_get(len)
        .array_new_default(array)
        .local_set(s)
        .local_get(negative)
        .if_(BlockType::Empty)
        .local_get(s)
        .i32_const(0)
        .i32_const('-' as i32)
        .array_set(array)
        .end()
        // From the last digit back to the first.
        .loop_(BlockType::Empty)
        .local_get(len)
        .i32_const(1)
        .i32_sub()
        .local_set(len)
        .local_get(s)
        .local_get(len)
        .local_get(n)
        .i64_const(10)
        .i64_rem_u()
        .i32_wrap_i64()
        .i32_const('0' as i32)
        .i32_add()
        .array_set(array)
        .local_get(n)
        .i64_const(10)
        .i64_div_u()
        .local_tee(n)
        .i64_const(0)
        .i64_ne()
        .br_if(0)
        .end()
        .local_get(s)
        .ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_concat(m: &mut ModuleState, array: u32) -> u32 {
    let ty = m.func_type(
        &[ref_to(array, false), ref_to(array, false)],
        &[ref_to(array, false)],
    );
    let (a, b) = (0, 1);
    let mut f = FuncState::new(2);
    let s = f.local(ref_to(array, true));
    f.ins()
        .local_get(a)
        .array_len()
        .local_get(b)
        .array_len()
        .i32_add()
        .array_new_default(array)
        .local_tee(s)
        .i32_const(0)
        .local_get(a)
        .i32_const(0)
        .local_get(a)
        .array_len()
        .array_copy(array, array)
        .local_get(s)
        .local_get(a)
        .array_len()
        .local_get(b)
        .i32_const(0)
        .local_get(b)
        .array_len()
        .array_copy(array, array)
        .local_get(s)
        .ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_compare(m: &mut ModuleState, array: u32) -> u32 {
    let ty = m.func_type(
        &[ref_to(array, false), ref_to(array, false)],
        &[ValType::I32],
    );
    let (a, b) = (0, 1);
    let mut f = FuncState::new(2);
    let i = f.local(ValType::I32);
    let shorter = f.local(ValType::I32);
    let x = f.local(ValType::I32);
    let y = f.local(ValType::I32);
    f.ins()
        .local_get(a)
        .array_len()
        .local_get(b)
        .array_len()
        .local_get(a)
        .array_len()
        .local_get(b)
        .array_len()
        .i32_lt_u()
        .select()
        .local_set(shorter)
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(i)
        .local_get(shorter)
        .i32_ge_u()
        .br_if(1)
        .local_get(a)
        .local_get(i)
        .array_get(array)
        .local_set(x)
        .local_get(b)
        .local_get(i)
        .array_get(array)
        .local_set(y)
        .local_get(x)
        .local_get(y)
        .i32_ne()
        .if_(BlockType::Empty)
        .local_get(x)
        .local_get(y)
        .i32_gt_u()
        .local_get(x)
        .local_get(y)
        .i32_lt_u()
        .i32_sub()
        .return_()
        .end()
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i)
        .br(0)
        .end()
        .end()
        // One `str` starts the other, and the shorter one comes first.
        .local_get(a)
        .array_len()
        .local_get(b)
        .array_len()
        .i32_gt_u()
        .local_get(a)
        .array_len()
        .local_get(b)
        .array_len()
        .i32_lt_u()
        .i32_sub();
    m.add_func(ty, f.finish())
}

fn add_repeat(m: &mut ModuleState, array: u32) -> u32 {
    let ty = m.func_type(
        &[ref_to(array, false), ValType::I32],
        &[ref_to(array, true)],
    );
    let (s, n) = (0, 1);
    let mut f = FuncState::new(2);
    let len = f.local(ValType::I64);
    let r = f.local(ref_to(array, true));
    let done = f.local(ValType::I32);
    let chunk = f.local(ValType::I32);
    f.ins()
        // A count below 1 gives the empty `str`.
        .local_get(n)
        .i32_const(0)
        .i32_le_s()
        .if_(BlockType::Empty)
        .i32_const(0)
        .array_new_default(array)
        .return_()
        .end()
        .local_get(s)
        .array_len()
        .i64_extend_i32_u()
        .local_get(n)
        .i64_extend_i32_u()
        .i64_mul()
        .local_tee(len)
        .i64_const(MAX_LEN)
        .i64_gt_u()
        .if_(BlockType::Empty)
        .ref_null(HeapType::Concrete(array))
        .return_()
        .end()
        .local_get(len)
        .i32_wrap_i64()
        .array_new_default(array)
        .local_tee(r)
        .i32_const(0)
        .local_get(s)
        .i32_const(0)
        .local_get(s)
        .array_len()
        .array_copy(array, array)
        .local_get(s)
        .array_len()
        .local_set(done)
        // Each copy doubles the part of the result that is done.
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(done)
        .i64_extend_i32_u()
        .local_get(len)
        .i64_ge_u()
        .br_if(1)
        .local_get(done)
        .local_get(len)
        .i32_wrap_i64()
        .local_get(done)
        .i32_sub()
        .local_get(done)
        .local_get(len)
        .i32_wrap_i64()
        .local_get(done)
        .i32_sub()
        .i32_lt_u()
        .select()
        .local_set(chunk)
        .local_get(r)
        .local_get(done)
        .local_get(r)
        .i32_const(0)
        .local_get(chunk)
        .array_copy(array, array)
        .local_get(done)
        .local_get(chunk)
        .i32_add()
        .local_set(done)
        .br(0)
        .end()
        .end()
        .local_get(r);
    m.add_func(ty, f.finish())
}
