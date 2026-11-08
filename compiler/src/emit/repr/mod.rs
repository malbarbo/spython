//! How each value is laid out in wasm. No other module knows it.

mod int;
pub(super) mod str;

use wasm_encoder::{BlockType, ConstExpr, HeapType, ValType};

use super::module::{FuncState, ModuleState};
use super::runtime::exc::Exceptions;
use super::runtime::host::Host;
use crate::cpython::{MEMORY_ERROR, STR_INDEX_OUT_OF_RANGE};
use crate::ir::{ArithOp, CmpOp, Rep, SiteFrame};
use int::IntRepr;
use str::StrRepr;

pub(crate) struct Reprs {
    int: IntRepr,
    str: StrRepr,
    exc: Exceptions,
}

impl Reprs {
    pub fn new(m: &mut ModuleState, host: &Host) -> Self {
        let mut str = StrRepr::ucs4(m, host);
        let exc = Exceptions::new(m, host, &mut str);
        let int = IntRepr::i31(m, host, &mut str, &exc);
        Reprs { int, str, exc }
    }

    /// Opens the code of the module or of a function, where the handler
    /// that `end_module` or `end_function` adds catches an exception.
    pub fn begin(&self, f: &mut FuncState) {
        self.exc.begin(f);
    }

    /// Closes what [`Self::begin`] opened in the module, where an exception
    /// that nothing catches writes the traceback and ends the program.
    /// `site` is the local that holds the site of the instruction that may
    /// raise.
    pub fn end_module(&self, f: &mut FuncState, site: u32) {
        self.exc.end_module(f, site);
    }

    /// Pushes 1 when a call has room for one more frame, and 0 when the call
    /// raises `RecursionError`.
    pub fn call_fits(&self, f: &mut FuncState) {
        self.exc.call_fits(f);
    }

    /// Counts the frame of the function that starts.
    pub fn enter(&self, f: &mut FuncState) {
        self.exc.enter(f);
    }

    /// Drops the frame of the function that returns.
    pub fn leave(&self, f: &mut FuncState) {
        self.exc.leave(f);
    }

    /// Closes what [`Self::begin`] opened in a function, where an exception
    /// goes on to the caller with the frame of the function.
    pub fn end_function(&self, f: &mut FuncState, site: u32) {
        self.exc.end_function(f, site);
    }

    /// Adds what the module needs once the program is emitted, with the
    /// text of the frame of each site.
    pub fn finish(&mut self, m: &mut ModuleState, sites: &[SiteFrame]) {
        self.exc.finish(m, &mut self.str, sites);
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

    /// The type of a value of `rep` on the stack, and `None` for a rep that
    /// has no value.
    pub fn value_type(&self, rep: Rep) -> Option<ValType> {
        match rep {
            Rep::Str => Some(self.str.val_type(false)),
            _ => self.local_type(rep),
        }
    }

    /// Pushes the value of `rep` that `local` holds.
    pub fn load(&self, f: &mut FuncState, rep: Rep, local: u32) {
        f.ins().local_get(local);
        if rep == Rep::Str {
            f.ins().ref_as_non_null();
        }
    }

    /// A global that holds a value of `rep`. Until the first `global.set`,
    /// it holds a value that no read sees.
    pub fn global(&self, m: &mut ModuleState, rep: Rep) -> u32 {
        let ty = self.local_type(rep).expect("a variable has a value");
        let init = match rep {
            Rep::Int => self.int.global_init(),
            Rep::Bool => ConstExpr::i32_const(0),
            Rep::Str => ConstExpr::ref_null(HeapType::Concrete(self.str.type_index())),
            Rep::None => unreachable!("a variable of rep None"),
        };
        m.global(ty, &init)
    }

    /// Pushes the value of `rep` that `global` holds.
    pub fn load_global(&self, f: &mut FuncState, rep: Rep, global: u32) {
        f.ins().global_get(global);
        if rep == Rep::Str {
            f.ins().ref_as_non_null();
        }
    }

    /// Raises an exception with `message`, the last line of its traceback.
    pub fn raise(&mut self, m: &mut ModuleState, f: &mut FuncState, message: &str) {
        self.str.literal(m, f, message);
        self.exc.raise(f);
    }

    /// Pops a `str` and raises an exception with it as the last line of its
    /// traceback.
    pub fn raise_message(&self, f: &mut FuncState) {
        self.exc.raise(f);
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

    /// Pops a value of `rep` and pushes its truth as a `bool`.
    pub fn truth(&self, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.truth(f),
            Rep::Bool => {}
            Rep::Str => self.str.truth(f),
            Rep::None => unreachable!("None has no value"),
        }
    }

    /// Pops two `str`s and pushes the second one after the first.
    pub fn str_concat(&self, f: &mut FuncState) {
        self.str.concat(f);
    }

    /// Pops a `bool` and pushes its negation.
    pub fn bool_not(&self, f: &mut FuncState) {
        f.ins().i32_eqz();
    }

    /// Pops a `bool` and pushes it as an `int`.
    pub fn bool_as_int(&self, f: &mut FuncState) {
        self.int.bool_as_int(f);
    }

    /// Pops two `int`s and pushes the result of `op`.
    pub fn int_arith(&self, f: &mut FuncState, op: ArithOp) {
        self.int.arith(f, op);
    }

    /// Pops a `str` and pushes its number of code points as an `int`.
    pub fn str_len(&self, f: &mut FuncState) {
        self.str.len(f);
        self.int.len_as_int(f);
    }

    /// Pops a `str` and an `int`, and pushes the `str` repeated that many
    /// times. A result too long for memory raises `MemoryError`.
    pub fn str_repeat(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.repeat(f);
        self.raise_if_null(m, f, MEMORY_ERROR);
    }

    /// Pops a `str` and an `int`, and pushes the code point at that index
    /// as a `str`. An index out of range raises `IndexError`.
    pub fn str_index(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.index(f);
        self.raise_if_null(m, f, STR_INDEX_OUT_OF_RANGE);
    }

    /// Raises an exception with `message` when the `str` on the stack is
    /// null, and leaves it as a `str` otherwise.
    fn raise_if_null(&mut self, m: &mut ModuleState, f: &mut FuncState, message: &str) {
        let result = f.local(self.str.val_type(true));
        f.ins()
            .local_tee(result)
            .ref_is_null()
            .if_(BlockType::Empty);
        self.raise(m, f, message);
        f.ins().end();
        self.load(f, Rep::Str, result);
    }

    /// Pops two values of `rep` and pushes the `bool` of `op`.
    pub fn compare(&self, f: &mut FuncState, rep: Rep, op: CmpOp) {
        match rep {
            Rep::Int => self.int.compare(f, op),
            // `a op b` is `compare(a, b) op 0`.
            Rep::Str => {
                self.str.compare(f);
                self.int.literal(f, 0);
                self.int.compare(f, op);
            }
            Rep::Bool | Rep::None => unreachable!("a comparison of a {rep:?}"),
        }
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
