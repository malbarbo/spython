//! How each value is laid out in wasm. No other module knows it.

mod int;
mod str;

use wasm_encoder::{BlockType, ValType};

use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};
use crate::ir::{ArithOp, CmpOp, Rep};
use int::IntRepr;
use str::StrRepr;

pub(crate) struct Reprs {
    int: IntRepr,
    str: StrRepr,
}

impl Reprs {
    pub fn new(m: &mut ModuleState, host: &Host) -> Self {
        let mut str = StrRepr::ucs4(m, host);
        let int = IntRepr::i31(m, host, &mut str);
        Reprs { int, str }
    }

    /// The type of a local that holds a value of `rep`, and `None` for a
    /// rep that has no value. A reference is nullable in a local, and
    /// [`Self::load`] reads it back.
    pub fn local_type(&self, rep: Rep) -> Option<ValType> {
        match rep {
            Rep::Int => Some(self.int.val_type()),
            // `False` and `True` are the `i32` 0 and 1.
            Rep::Bool => Some(ValType::I32),
            Rep::Str => Some(self.str.val_type(true)),
            Rep::None => None,
        }
    }

    /// Pushes the value of `rep` that `local` holds.
    pub fn load(&self, f: &mut FuncState, rep: Rep, local: u32) {
        f.ins().local_get(local);
        if rep == Rep::Str {
            f.ins().ref_as_non_null();
        }
    }

    /// Pushes the `str` with the text `s`.
    pub fn str_literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        self.str.literal(m, f, s);
    }

    pub fn int_literal(&self, f: &mut FuncState, value: i32) {
        self.int.literal(f, value);
    }

    pub fn bool_literal(&self, f: &mut FuncState, value: bool) {
        f.ins().i32_const(i32::from(value));
    }

    /// Pops a `bool` and pushes it as an `int`.
    pub fn bool_as_int(&self, f: &mut FuncState) {
        self.int.bool_as_int(f);
    }

    /// Pops two `int`s and pushes the result of `op`.
    pub fn int_arith(&self, f: &mut FuncState, op: ArithOp) {
        self.int.arith(f, op);
    }

    /// Pops two `int`s and pushes the `bool` of `op`.
    pub fn int_compare(&self, f: &mut FuncState, op: CmpOp) {
        self.int.compare(f, op);
    }

    /// Pops a value of `rep` and writes what `str` gives for it to the
    /// standard output.
    pub fn write(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.write(f),
            Rep::Bool => {
                f.ins().if_(BlockType::Empty);
                self.str.literal(m, f, "True");
                self.str.write(f);
                f.ins().else_();
                self.str.literal(m, f, "False");
                self.str.write(f);
                f.ins().end();
            }
            Rep::Str => self.str.write(f),
            Rep::None => panic!("a value of rep None to write"),
        }
    }
}
