//! The representations of `float`.

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, ValType};

use crate::emit::module::{FuncIdx, FuncState, ModuleState};
use crate::emit::runtime::host::Host;

pub(super) enum FloatRepr {
    /// An `f64`.
    F64 {
        /// `(func (param f64))`, which writes the `repr` of the `float`. A
        /// module that writes no `float` goes without it, and without the
        /// builtin that it calls.
        write: Option<FuncIdx>,
        write_char: FuncIdx,
        end_write: FuncIdx,
        scratch: i32,
    },
}

impl FloatRepr {
    pub fn f64(host: &Host) -> Self {
        FloatRepr::F64 {
            write: None,
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

    /// Pops a `float` and writes its `repr` to the standard output, as one
    /// write.
    pub fn write(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        match self {
            FloatRepr::F64 {
                write,
                write_char,
                end_write,
                scratch,
            } => {
                let write =
                    *write.get_or_insert_with(|| add_write(m, *write_char, *end_write, *scratch));
                f.ins().call(write);
            }
        }
    }
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
