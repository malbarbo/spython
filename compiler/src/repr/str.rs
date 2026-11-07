//! The representations of `str`.

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, HeapType, RefType, StorageType, ValType};

use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};

pub(super) enum StrRepr {
    /// One code point per element of an `(array i32)`, so `s[i]` is O(1).
    Ucs4 {
        array: u32,
        /// `(func (param (ref $Str)))`, which writes the `str` to the
        /// standard output.
        write: u32,
        /// The global that holds each literal, by its text.
        literals: HashMap<String, u32>,
    },
}

impl StrRepr {
    pub fn ucs4(m: &mut ModuleState, host: &Host) -> Self {
        let array = m.array_type(StorageType::Val(ValType::I32), false);
        let write = add_write(m, host, array);
        StrRepr::Ucs4 {
            array,
            write,
            literals: HashMap::new(),
        }
    }

    /// Pushes the literal `s`. Each literal is built once, by `_start`
    /// before the program runs, and kept in a global.
    pub fn literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        match self {
            StrRepr::Ucs4 {
                array, literals, ..
            } => {
                let array = *array;
                let global = *literals.entry(s.to_owned()).or_insert_with(|| {
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
                });
                f.ins().global_get(global).ref_as_non_null();
            }
        }
    }

    pub fn val_type(&self, nullable: bool) -> ValType {
        match self {
            StrRepr::Ucs4 { array, .. } => ref_to(*array, nullable),
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
