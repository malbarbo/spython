//! The representations of `str`.

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, StorageType, ValType};

use crate::emit::module::{DataIdx, FuncIdx, FuncState, GlobalIdx, ModuleState, TypeIdx};
use crate::emit::runtime::host::{self, Host};

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
        /// `(func (param i32) (result (ref $Str)))`, the `str` of that many
        /// ASCII bytes of the scratch area of the host, which the module
        /// gets at the first use.
        from_scratch: Option<FuncIdx>,
        /// The address of the scratch area of the host.
        scratch: i32,
        /// The global that holds each literal, by its text.
        literals: HashMap<String, GlobalIdx>,
        /// `(func (param (ref $Str) i32 i32) (result (ref $Str)))`, which
        /// gives the slice between the two indices, as `s[start:stop]`. The
        /// module gets it at the first use.
        slice: Option<FuncIdx>,
        /// `(func (result (ref $Str)))`, which gives what the program wrote
        /// to the buffer of its captured output, and empties the buffer. A
        /// program that captures its output has it.
        captured: Option<FuncIdx>,
        /// `(func (param (ref $Str)) (result i64))`, the `hash` of the
        /// `str`, which the module gets at the first use.
        hash: Option<FuncIdx>,
        /// The address of [`Host::free`].
        free: i32,
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
            from_scratch: None,
            scratch: host.scratch,
            literals: HashMap::new(),
            slice: None,
            captured: host
                .capture
                .map(|capture| add_capture(m, array, capture.char)),
            hash: None,
            free: host.free,
        }
    }

    /// Pushes what the program wrote to the buffer of its captured output,
    /// and empties the buffer.
    pub fn captured(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { captured, .. } => {
                f.ins()
                    .call(captured.expect("a program that captures its output"));
            }
        }
    }

    /// Pushes the literal `s`.
    pub fn literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        let global = self.literal_global(m, s);
        f.ins().global_get(global).ref_as_non_null();
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
                    let data = m.passive_data(encode(s));
                    let global =
                        m.global(array.ref_to(true), &ConstExpr::ref_null(array.heap_type()));
                    let len =
                        i32::try_from(s.chars().count()).expect("a literal shorter than 2^31");
                    m.init_value()
                        .i32_const(0)
                        .i32_const(len)
                        .array_new_data(array, data)
                        .global_set(global)
                        .data_drop(data);
                    global
                })
            }
        }
    }

    /// The bytes of `s` in a data segment that [`Self::new_data`] reads.
    pub fn encode(&self, s: &str) -> Vec<u8> {
        match self {
            StrRepr::Ucs4 { .. } => encode(s),
        }
    }

    /// Pops the start and the length of a text in `data`, which
    /// [`Self::encode`] wrote, and pushes its `str`. The start counts bytes
    /// and the length counts code points.
    pub fn new_data(&self, f: &mut FuncState, data: DataIdx) {
        match self {
            StrRepr::Ucs4 { array, .. } => {
                f.ins().array_new_data(*array, data);
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
                f.ins().call(*ascii);
            }
        }
    }

    /// Pops a length and pushes the `str` of that many ASCII bytes of the
    /// scratch area of the host, where a builtin wrote them.
    pub fn scratch_ascii(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 {
                array,
                from_scratch,
                scratch,
                ..
            } => {
                let from_scratch =
                    *from_scratch.get_or_insert_with(|| add_from_scratch(m, *array, *scratch));
                f.ins().call(from_scratch);
            }
        }
    }

    pub fn val_type(&self, nullable: bool) -> ValType {
        match self {
            StrRepr::Ucs4 { array, .. } => array.ref_to(nullable),
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

    /// Pops a `str` and pushes its number of code points as an `i32`
    /// without sign.
    pub fn len(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { .. } => {
                f.ins().array_len();
            }
        }
    }

    /// Pops a `str` and two `i32`, and pushes the slice of the `str` from
    /// the first index up to the second, as `s[start:stop]` in Python.
    pub fn slice(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { array, slice, .. } => {
                let slice = *slice.get_or_insert_with(|| add_slice(m, *array));
                f.ins().call(slice);
            }
        }
    }

    /// Pops an `i32` and pushes a new `str` of that many code points, each
    /// of them 0.
    pub fn new_zeroed(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { array, .. } => {
                f.ins().array_new_default(*array);
            }
        }
    }

    /// Pops a `str`, an `i32` index in range and an `i32` code point, and
    /// sets the code point of the `str` at the index.
    pub fn set(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { array, .. } => {
                f.ins().array_set(*array);
            }
        }
    }

    /// Pops a `str` and an `i32` index in range, and pushes the code point
    /// at the index.
    pub fn get(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { array, .. } => {
                f.ins().array_get(*array);
            }
        }
    }

    /// Pops a `str` and pushes its `hash`, an `i64`.
    pub fn hash(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 {
                array, hash, free, ..
            } => {
                let hash = *hash.get_or_insert_with(|| add_hash(m, *array, *free));
                f.ins().call(hash);
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

    /// Pops a `str` and an `i32`, and pushes the code point at that index
    /// as a `str`, or null when the index is out of range.
    pub fn index(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { index, .. } => {
                f.ins().call(*index);
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

    /// Pops a `str` and writes it, as one call of `write` in CPython.
    pub fn write(&self, f: &mut FuncState) {
        match self {
            StrRepr::Ucs4 { write, .. } => {
                f.ins().call(*write);
            }
        }
    }
}

/// Defines `char`, which adds a code point to the buffer of the captured
/// output, and returns the function that gives the buffer as a `str` and
/// empties it. The buffer is an array that doubles when it fills.
fn add_capture(m: &mut ModuleState, array: TypeIdx, char: FuncIdx) -> FuncIdx {
    let buffer = m.global(array.ref_to(true), &ConstExpr::ref_null(array.heap_type()));
    let len = m.global(ValType::I32, &ConstExpr::i32_const(0));
    m.init_value()
        .i32_const(64)
        .array_new_default(array)
        .global_set(buffer);

    let mut f = FuncState::new(1);
    let c = f.param(0);
    let grown = f.local(array.ref_to(true));
    f.ins()
        .global_get(len)
        .global_get(buffer)
        .ref_as_non_null()
        .array_len()
        .i32_eq();
    f.if_(BlockType::Empty);
    f.ins()
        .global_get(len)
        .i32_const(2)
        .i32_mul()
        .array_new_default(array)
        .local_tee(grown)
        .i32_const(0)
        .global_get(buffer)
        .ref_as_non_null()
        .i32_const(0)
        .global_get(len)
        .array_copy(array, array)
        .local_get(grown)
        .global_set(buffer);
    f.end();
    f.ins()
        .global_get(buffer)
        .ref_as_non_null()
        .global_get(len)
        .local_get(c)
        .array_set(array)
        .global_get(len)
        .i32_const(1)
        .i32_add()
        .global_set(len);
    m.define(char, f.finish());

    let ty = m.func_type(&[], &[array.ref_to(false)]);
    let mut f = FuncState::new(0);
    let s = f.local(array.ref_to(true));
    f.ins()
        .global_get(len)
        .array_new_default(array)
        .local_tee(s)
        .i32_const(0)
        .global_get(buffer)
        .ref_as_non_null()
        .i32_const(0)
        .global_get(len)
        .array_copy(array, array)
        .i32_const(0)
        .global_set(len)
        .local_get(s)
        .ref_as_non_null();
    m.add_func(ty, f.finish())
}

fn add_from_scratch(m: &mut ModuleState, array: TypeIdx, scratch: i32) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[array.ref_to(false)]);
    let mut f = FuncState::new(1);
    let len = f.param(0);
    let i = f.local(ValType::I32);
    let s = f.local(array.ref_to(true));
    f.ins().local_get(len).array_new_default(array).local_set(s);
    let exit = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(len).i32_ge_u();
    f.br_if(exit);
    f.ins()
        .local_get(s)
        .local_get(i)
        .local_get(i)
        .i32_load8_u(MemArg {
            offset: u64::try_from(scratch).expect("a positive address"),
            align: 0,
            target: MemArgTarget::Memory(0),
            ordering: None,
        })
        .array_set(array)
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

fn add_ascii(m: &mut ModuleState, array: TypeIdx, bytes: TypeIdx) -> FuncIdx {
    let ty = m.func_type(&[bytes.ref_to(false)], &[array.ref_to(false)]);
    let mut f = FuncState::new(1);
    let text = f.param(0);
    let s = f.local(array.ref_to(true));
    let i = f.local(ValType::I32);
    f.ins()
        .local_get(text)
        .array_len()
        .array_new_default(array)
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
        .array_get_u(bytes)
        .array_set(array)
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
    let ty = m.func_type(&[array.ref_to(false)], &[]);
    let mut f = FuncState::new(1);
    let s = f.param(0);
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
        .array_get(array)
        .call(host.write_char)
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i);
    f.br(again);
    f.end();
    f.end();
    f.ins().call(host.end_write);
    m.add_func(ty, f.finish())
}

/// The function that gives the `hash` of a `str`. It copies the code
/// points to the free memory at `free`, 4 bytes each, and `str_hash` reads
/// them there.
fn add_hash(m: &mut ModuleState, array: TypeIdx, free: i32) -> FuncIdx {
    let builtin = m.builtin("str_hash");
    let ty = m.func_type(&[array.ref_to(false)], &[ValType::I64]);
    let mut f = FuncState::new(1);
    let s = f.param(0);
    let (len, i) = (f.local(ValType::I32), f.local(ValType::I32));
    f.ins()
        .local_get(s)
        .array_len()
        .local_tee(len)
        .i32_const(2)
        .i32_shl();
    host::reserve(&mut f, free);
    let done = f.block(BlockType::Empty);
    let again = f.loop_(BlockType::Empty);
    f.ins().local_get(i).local_get(len).i32_ge_u();
    f.br_if(done);
    f.ins()
        .local_get(i)
        .i32_const(2)
        .i32_shl()
        .local_get(s)
        .local_get(i)
        .array_get(array)
        .i32_store(MemArg {
            offset: u64::try_from(free).expect("a positive address"),
            align: 2,
            target: MemArgTarget::Memory(0),
            ordering: None,
        })
        .local_get(i)
        .i32_const(1)
        .i32_add()
        .local_set(i);
    f.br(again);
    f.end();
    f.end();
    f.ins().i32_const(free).local_get(len).call(builtin);
    m.add_func(ty, f.finish())
}

fn add_decimal(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I64], &[array.ref_to(false)]);
    let mut f = FuncState::new(1);
    let v = f.param(0);
    let negative = f.local(ValType::I32);
    let n = f.local(ValType::I64);
    let len = f.local(ValType::I32);
    let rest = f.local(ValType::I64);
    let s = f.local(array.ref_to(true));
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
        .array_new_default(array)
        .local_set(s)
        .local_get(negative);
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(s)
        .i32_const(0)
        .i32_const('-' as i32)
        .array_set(array);
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
        .array_set(array)
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
        &[array.ref_to(false), array.ref_to(false)],
        &[array.ref_to(false)],
    );
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
    let s = f.local(array.ref_to(true));
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

fn add_compare(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
    let ty = m.func_type(&[array.ref_to(false), array.ref_to(false)], &[ValType::I32]);
    let mut f = FuncState::new(2);
    let (a, b) = (f.param(0), f.param(1));
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
        .array_get(array)
        .local_set(x)
        .local_get(b)
        .local_get(i)
        .array_get(array)
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
    let ty = m.func_type(&[array.ref_to(false), ValType::I32], &[array.ref_to(true)]);
    let mut f = FuncState::new(2);
    let (s, n) = (f.param(0), f.param(1));
    let len = f.local(ValType::I64);
    let r = f.local(array.ref_to(true));
    let done = f.local(ValType::I32);
    let chunk = f.local(ValType::I32);
    f.ins()
        // A count below 1 gives the empty `str`.
        .local_get(n)
        .i32_const(0)
        .i32_le_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).array_new_default(array).return_();
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
    f.ins().ref_null(array.heap_type()).return_();
    f.end();
    f.ins()
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
        .array_copy(array, array)
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

/// The code points of `s` in UCS-4, little endian.
fn encode(s: &str) -> Vec<u8> {
    s.chars().flat_map(|c| (c as u32).to_le_bytes()).collect()
}

/// The function of `s[start:stop]`. An index below 0 counts from the end,
/// and each index stays between 0 and the length, as in Python.
fn add_slice(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
    let ty = m.func_type(
        &[array.ref_to(false), ValType::I32, ValType::I32],
        &[array.ref_to(false)],
    );
    let mut f = FuncState::new(3);
    let (s, start, stop) = (f.param(0), f.param(1), f.param(2));
    let len = f.local(ValType::I32);
    let result = f.local(array.ref_to(true));
    f.ins().local_get(s).array_len().local_set(len);
    for index in [start, stop] {
        f.ins().local_get(index).i32_const(0).i32_lt_s();
        f.if_(BlockType::Empty);
        f.ins()
            .local_get(index)
            .local_get(len)
            .i32_add()
            .local_tee(index)
            .i32_const(0)
            .i32_lt_s();
        f.if_(BlockType::Empty);
        f.ins().i32_const(0).local_set(index);
        f.end();
        f.else_();
        f.ins().local_get(index).local_get(len).i32_gt_s();
        f.if_(BlockType::Empty);
        f.ins().local_get(len).local_set(index);
        f.end();
        f.end();
    }
    // A `stop` before the `start` gives an empty `str`.
    f.ins().local_get(stop).local_get(start).i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins().local_get(start).local_set(stop);
    f.end();
    f.ins()
        .local_get(stop)
        .local_get(start)
        .i32_sub()
        .array_new_default(array)
        .local_tee(result)
        .i32_const(0)
        .local_get(s)
        .local_get(start)
        .local_get(stop)
        .local_get(start)
        .i32_sub()
        .array_copy(array, array)
        .local_get(result)
        .ref_as_non_null();
    m.add_func(ty, f.finish())
}

/// The code points below this one each have one `str`, which the first
/// index that gives the code point builds.
const SHARED_CHARS: i32 = 256;

fn add_index(m: &mut ModuleState, array: TypeIdx) -> FuncIdx {
    let table = m.array_type(StorageType::Val(array.ref_to(true)), true);
    let shared = m.global(table.ref_to(true), &ConstExpr::ref_null(table.heap_type()));
    let ty = m.func_type(&[array.ref_to(false), ValType::I32], &[array.ref_to(true)]);
    let mut f = FuncState::new(2);
    let (s, i) = (f.param(0), f.param(1));
    let c = f.local(ValType::I32);
    let one = f.local(array.ref_to(true));
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
    f.ins().ref_null(array.heap_type()).return_();
    f.end();
    f.ins()
        .local_get(s)
        .local_get(i)
        .array_get(array)
        .local_tee(c)
        .i32_const(SHARED_CHARS)
        .i32_ge_u();
    f.if_(BlockType::Empty);
    f.ins().local_get(c).array_new_fixed(array, 1).return_();
    f.end();
    // The first index builds the table, so a program that never indexes
    // never builds it.
    f.ins().global_get(shared).ref_is_null();
    f.if_(BlockType::Empty);
    f.ins()
        .i32_const(SHARED_CHARS)
        .array_new_default(table)
        .global_set(shared);
    f.end();
    f.ins()
        .global_get(shared)
        .local_get(c)
        .array_get(table)
        .local_tee(one)
        .ref_is_null();
    f.if_(BlockType::Empty);
    f.ins()
        .global_get(shared)
        .local_get(c)
        .local_get(c)
        .array_new_fixed(array, 1)
        .local_tee(one)
        .array_set(table);
    f.end();
    f.ins().local_get(one);
    m.add_func(ty, f.finish())
}
