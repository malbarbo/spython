//! The representations of `str`.

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, HeapType, RefType, StorageType, ValType};

use crate::emit::module::{FuncIdx, FuncState, GlobalIdx, ModuleState, TypeIdx};
use crate::emit::runtime::host::Host;

/// The most code points in a `str`, the longest array that an `i32`
/// length gives.
const MAX_LEN: i64 = i32::MAX as i64;

pub(in crate::emit) enum StrRepr {
    /// One code point per element of an `(array (mut i32))`, so `s[i]` is
    /// O(1). The array is mutable only while a function builds it.
    Ucs4 {
        array: TypeIdx,
        /// `(func (param (ref $Str)))`, which writes the `str` to the
        /// standard output.
        write: FuncIdx,
        /// `(func (param i64) (result (ref $Str)))`, the decimal digits.
        decimal: FuncIdx,
        /// `(func (param (ref $Str) (ref $Str)) (result (ref $Str)))`.
        concat: FuncIdx,
        /// `(func (param (ref $Str) i32) (result (ref null $Str)))`, which
        /// repeats the `str`, and gives null when the result is too long.
        repeat: FuncIdx,
        /// `(func (param (ref $Str) i32) (result (ref null $Str)))`, which
        /// gives the code point at the index as a `str`, from the end when
        /// the index is negative, and null when the index is out of range.
        index: FuncIdx,
        /// `(func (param (ref $Str) (ref $Str)) (result i32))`, which gives
        /// -1, 0 or 1 as the first `str` comes before, with or after the
        /// second one.
        compare: FuncIdx,
        /// `(array i8)`, the bytes of an ASCII text.
        bytes: TypeIdx,
        /// `(func (param (ref $Bytes)) (result (ref $Str)))`, the `str` of
        /// an ASCII text.
        ascii: FuncIdx,
        /// The global that holds each literal, by its text.
        literals: HashMap<String, GlobalIdx>,
    },
}

impl StrRepr {
    pub fn ucs4(m: &mut ModuleState, host: &Host) -> Self {
        let array = m.array_type(StorageType::Val(ValType::I32), true);
        let write = add_write(m, host, array);
        let bytes = m.array_type(StorageType::I8, false);
        StrRepr::Ucs4 {
            array,
            write,
            decimal: add_decimal(m, array),
            concat: add_concat(m, array),
            repeat: add_repeat(m, array),
            index: add_index(m, array),
            compare: add_compare(m, array),
            bytes,
            ascii: add_ascii(m, array, bytes),
            literals: HashMap::new(),
        }
    }

    /// Pushes the literal `s`.
    pub fn literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        let global = self.literal_global(m, s);
        f.ins().global_get(global.0).ref_as_non_null();
    }

    /// The global that holds the literal `s`. Each literal is built once,
    /// by `_start` before the program runs.
    pub fn literal_global(&mut self, m: &mut ModuleState, s: &str) -> GlobalIdx {
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
                        &ConstExpr::ref_null(HeapType::Concrete(array.0)),
                    );
                    let len =
                        i32::try_from(s.chars().count()).expect("a literal shorter than 2^31");
                    wasm_encoder::InstructionSink::new(&mut m.init)
                        .i32_const(0)
                        .i32_const(len)
                        .array_new_data(array.0, data.0)
                        .global_set(global.0);
                    global
                })
            }
        }
    }

    /// The index of the array type of a `str`.
    pub fn type_index(&self) -> TypeIdx {
        match self {
            StrRepr::Ucs4 { array, .. } => *array,
        }
    }

    /// The index of `(array i8)`, the type that [`Self::ascii`] takes.
    pub fn bytes_type(&self) -> TypeIdx {
        match self {
            StrRepr::Ucs4 { bytes, .. } => *bytes,
        }
    }

    /// Pops the bytes of an ASCII text and pushes its `str`. A text that
    /// only a run time error needs is cheaper as bytes than as a literal.
    pub fn ascii(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { ascii, .. } => {
                f.ins().call(ascii.0);
            }
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
                f.ins().call(decimal.0);
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

    /// Pops a `str` and pushes its number of code points as an `i32`
    /// without sign.
    pub fn len(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { .. } => {
                f.ins().array_len();
            }
        }
    }

    /// Pops two `str`s and pushes the second one after the first.
    pub fn concat(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { concat, .. } => {
                f.ins().call(concat.0);
            }
        }
    }

    /// Pops a `str` and an `i32`, and pushes the `str` repeated that many
    /// times, or null when the result has more than [`MAX_LEN`] code
    /// points.
    pub fn repeat(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { repeat, .. } => {
                f.ins().call(repeat.0);
            }
        }
    }

    /// Pops a `str` and an `i32`, and pushes the code point at that index
    /// as a `str`, or null when the index is out of range.
    pub fn index(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { index, .. } => {
                f.ins().call(index.0);
            }
        }
    }

    /// Pops two `str`s and pushes -1, 0 or 1 as the first one comes
    /// before, with or after the second one, by the order of their code
    /// points.
    pub fn compare(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { compare, .. } => {
                f.ins().call(compare.0);
            }
        }
    }

    pub fn write(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { write, .. } => {
                f.ins().call(write.0);
            }
        }
    }
}

fn ref_to(index: TypeIdx, nullable: bool) -> ValType {
    ValType::Ref(RefType {
        nullable,
        heap_type: HeapType::Concrete(index.0),
    })
}

fn add_ascii(m: &mut ModuleState, array: TypeIdx, bytes: TypeIdx) -> FuncIdx {
    let ty = m.func_type(&[ref_to(bytes, false)], &[ref_to(array, false)]);
    let text = 0;
    let mut f = FuncState::new(1);
    let s = f.local(ref_to(array, true));
    let i = f.local(ValType::I32);
    f.ins()
        .local_get(text)
        .array_len()
        .array_new_default(array.0)
        .local_set(s);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(text).array_len().i32_ge_u();
    f.br_if(exit);
    f.ins()
        .local_get(s)
        .local_get(i)
        .local_get(text)
        .local_get(i)
        .array_get_u(bytes.0)
        .array_set(array.0)
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i);
    f.br(again);
    f.end();
    f.end();
    f.ins().local_get(s).ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_write(m: &mut ModuleState, host: &Host, array: TypeIdx) -> FuncIdx {
    let ty = m.func_type(&[ref_to(array, false)], &[]);
    let s = 0;
    let mut f = FuncState::new(1);
    let i = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    f.ins().local_get(s).array_len().local_set(len);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(len).i32_ge_u();
    f.br_if(exit);
    f.ins()
        .local_get(s)
        .local_get(i)
        .array_get(array.0)
        .call(host.write_char.0)
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i);
    f.br(again);
    f.end();
    f.end();
    m.add_func(ty, f.finish())
}

fn add_decimal(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
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
        .local_get(negative);
    f.if_(BlockType::Empty);
    f.ins().i64_const(0).local_get(v).i64_sub().local_set(n);
    f.end();
    f.ins()
        // The digits, one more for each division by 10 that leaves a rest.
        .local_get(negative)
        .i32_const(1)
        .i32_add()
        .local_set(len)
        .local_get(n)
        .i64_const(10)
        .i64_div_u()
        .local_set(rest);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(rest).i64_eqz();
    f.br_if(exit);
    f.ins()
        .local_get(len)
        .i32_const(1)
        .i32_add()
        .local_set(len)
        .local_get(rest)
        .i64_const(10)
        .i64_div_u()
        .local_set(rest);
    f.br(again);
    f.end();
    f.end();
    f.ins()
        .local_get(len)
        .array_new_default(array.0)
        .local_set(s)
        .local_get(negative);
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(s)
        .i32_const(0)
        .i32_const('-' as i32)
        .array_set(array.0);
    f.end();
    // From the last digit back to the first.
    let again = f.loop_(BlockType::Empty);
    f.ins()
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
        .array_set(array.0)
        .local_get(n)
        .i64_const(10)
        .i64_div_u()
        .local_tee(n)
        .i64_const(0)
        .i64_ne();
    f.br_if(again);
    f.end();
    f.ins().local_get(s).ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_concat(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
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
        .array_new_default(array.0)
        .local_tee(s)
        .i32_const(0)
        .local_get(a)
        .i32_const(0)
        .local_get(a)
        .array_len()
        .array_copy(array.0, array.0)
        .local_get(s)
        .local_get(a)
        .array_len()
        .local_get(b)
        .i32_const(0)
        .local_get(b)
        .array_len()
        .array_copy(array.0, array.0)
        .local_get(s)
        .ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_compare(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
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
        .local_set(shorter);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(shorter).i32_ge_u();
    f.br_if(exit);
    f.ins()
        .local_get(a)
        .local_get(i)
        .array_get(array.0)
        .local_set(x)
        .local_get(b)
        .local_get(i)
        .array_get(array.0)
        .local_set(y)
        .local_get(x)
        .local_get(y)
        .i32_ne();
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(x)
        .local_get(y)
        .i32_gt_u()
        .local_get(x)
        .local_get(y)
        .i32_lt_u()
        .i32_sub()
        .return_();
    f.end();
    f.ins().local_get(i).i32_const(1).i32_add().local_set(i);
    f.br(again);
    f.end();
    f.end();
    f.ins()
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

fn add_repeat(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
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
        .i32_le_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).array_new_default(array.0).return_();
    f.end();
    f.ins()
        .local_get(s)
        .array_len()
        .i64_extend_i32_u()
        .local_get(n)
        .i64_extend_i32_u()
        .i64_mul()
        .local_tee(len)
        .i64_const(MAX_LEN)
        .i64_gt_u();
    f.if_(BlockType::Empty);
    f.ins().ref_null(HeapType::Concrete(array.0)).return_();
    f.end();
    f.ins()
        .local_get(len)
        .i32_wrap_i64()
        .array_new_default(array.0)
        .local_tee(r)
        .i32_const(0)
        .local_get(s)
        .i32_const(0)
        .local_get(s)
        .array_len()
        .array_copy(array.0, array.0)
        .local_get(s)
        .array_len()
        .local_set(done);
    // Each copy doubles the part of the result that is done.
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins()
        .local_get(done)
        .i64_extend_i32_u()
        .local_get(len)
        .i64_ge_u();
    f.br_if(exit);
    f.ins()
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
        .array_copy(array.0, array.0)
        .local_get(done)
        .local_get(chunk)
        .i32_add()
        .local_set(done);
    f.br(again);
    f.end();
    f.end();
    f.ins().local_get(r);
    m.add_func(ty, f.finish())
}

/// The code points below this one each have one `str`, which the first
/// index that gives the code point builds.
const SHARED_CHARS: i32 = 256;

fn add_index(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
    let table = m.array_type(StorageType::Val(ref_to(array, true)), true);
    let shared = m.global(
        ref_to(table, true),
        &ConstExpr::ref_null(HeapType::Concrete(table.0)),
    );
    wasm_encoder::InstructionSink::new(&mut m.init)
        .i32_const(SHARED_CHARS)
        .array_new_default(table.0)
        .global_set(shared.0);
    let ty = m.func_type(
        &[ref_to(array, false), ValType::I32],
        &[ref_to(array, true)],
    );
    let (s, i) = (0, 1);
    let mut f = FuncState::new(2);
    let c = f.local(ValType::I32);
    let one = f.local(ref_to(array, true));
    f.ins().local_get(i).i32_const(0).i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(i)
        .local_get(s)
        .array_len()
        .i32_add()
        .local_set(i);
    f.end();
    f.ins()
        // A negative index that stays negative is out of range too.
        .local_get(i)
        .local_get(s)
        .array_len()
        .i32_ge_u();
    f.if_(BlockType::Empty);
    f.ins().ref_null(HeapType::Concrete(array.0)).return_();
    f.end();
    f.ins()
        .local_get(s)
        .local_get(i)
        .array_get(array.0)
        .local_tee(c)
        .i32_const(SHARED_CHARS)
        .i32_ge_u();
    f.if_(BlockType::Empty);
    f.ins().local_get(c).array_new_fixed(array.0, 1).return_();
    f.end();
    f.ins()
        .global_get(shared.0)
        .local_get(c)
        .array_get(table.0)
        .local_tee(one)
        .ref_is_null();
    f.if_(BlockType::Empty);
    f.ins()
        .global_get(shared.0)
        .local_get(c)
        .local_get(c)
        .array_new_fixed(array.0, 1)
        .local_tee(one)
        .array_set(table.0);
    f.end();
    f.ins().local_get(one);
    m.add_func(ty, f.finish())
}
