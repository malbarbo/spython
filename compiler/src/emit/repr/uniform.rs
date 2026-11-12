//! The uniform representation, which holds a value of any rep in a
//! `(ref null eq)`. An `int` is an `i31ref`, a `float` is a `$Float`, a
//! `bool` is one of the two `$Bool`, a `str` is its `$Str`, and `None` is
//! null.

use wasm_encoder::{AbstractHeapType, BlockType, HeapType, RefType, StorageType, ValType};

use super::super::module::{FuncState, GlobalIdx, ModuleState, TypeIdx};
use crate::ir::Rep;

pub(super) const EQ: HeapType = HeapType::Abstract {
    shared: false,
    ty: AbstractHeapType::Eq,
};

pub(super) struct UniformRepr {
    float: Option<TypeIdx>,
    bool: Option<Bools>,
}

/// `False` and `True`, which the module makes once.
struct Bools {
    ty: TypeIdx,
    false_: GlobalIdx,
    true_: GlobalIdx,
}

impl UniformRepr {
    pub fn new() -> Self {
        UniformRepr {
            float: None,
            bool: None,
        }
    }

    pub fn val_type(&self) -> ValType {
        ValType::Ref(RefType {
            nullable: true,
            heap_type: EQ,
        })
    }

    /// Pushes `None`.
    pub fn none(&self, f: &mut FuncState) {
        f.ins().ref_null(EQ);
    }

    /// Pops an `int` and pushes it boxed.
    pub fn box_int(&self, f: &mut FuncState) {
        f.ins().ref_i31();
    }

    /// Pops an `f64` and pushes it boxed.
    pub fn box_float(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let float = self.float_type(m);
        f.ins().struct_new(float);
    }

    fn float_type(&mut self, m: &mut ModuleState) -> TypeIdx {
        *self
            .float
            .get_or_insert_with(|| m.struct_type(&[(StorageType::Val(ValType::F64), false)]))
    }

    /// The rep of a boxed value of a type other than `str`, with the type
    /// of its box. `None` and a `str` have no box of their own.
    pub fn boxes(&mut self, m: &mut ModuleState) -> [(Rep, HeapType); 3] {
        [Rep::Int, Rep::Float, Rep::Bool].map(|rep| (rep, self.box_type(m, rep)))
    }

    /// The type of the box of a value of `rep`, an `int`, a `float` or a
    /// `bool`.
    pub fn box_type(&mut self, m: &mut ModuleState, rep: Rep) -> HeapType {
        match rep {
            Rep::Int => HeapType::Abstract {
                shared: false,
                ty: AbstractHeapType::I31,
            },
            Rep::Float => self.float_type(m).heap_type(),
            Rep::Bool => self.bool_type(m).heap_type(),
            _ => unreachable!("a box of {rep:?}"),
        }
    }

    /// Pops a boxed `int` and pushes it as an `i32`.
    pub fn unbox_int(&self, f: &mut FuncState) {
        let i31 = HeapType::Abstract {
            shared: false,
            ty: AbstractHeapType::I31,
        };
        f.ins().ref_cast(i31, false).i31_get_s();
    }

    /// Pops a boxed `float` and pushes it as an `f64`.
    pub fn unbox_float(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let float = self.float_type(m);
        f.ins()
            .ref_cast(float.heap_type(), false)
            .struct_get(float, 0);
    }

    /// Pops a boxed `bool` and pushes it as an `i32`.
    pub fn unbox_bool(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let true_ = self.bools(m).true_;
        f.ins().global_get(true_).ref_eq();
    }

    /// Pops a `bool` and pushes it boxed.
    pub fn box_bool(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let bools = self.bools(m);
        let (false_, true_) = (bools.false_, bools.true_);
        f.if_(BlockType::Result(self.val_type()));
        f.ins().global_get(true_);
        f.else_();
        f.ins().global_get(false_);
        f.end();
    }

    fn bool_type(&mut self, m: &mut ModuleState) -> TypeIdx {
        self.bools(m).ty
    }

    fn bools(&mut self, m: &mut ModuleState) -> &Bools {
        self.bool.get_or_insert_with(|| {
            // The field tells the two apart from a `$Float` and from each
            // other.
            let ty = m.struct_type(&[(StorageType::Val(ValType::I32), false)]);
            let mut global = |value| {
                let global = m.global(
                    ty.ref_to(true),
                    &wasm_encoder::ConstExpr::ref_null(ty.heap_type()),
                );
                m.init_value()
                    .i32_const(value)
                    .struct_new(ty)
                    .global_set(global);
                global
            };
            Bools {
                ty,
                false_: global(0),
                true_: global(1),
            }
        })
    }
}
