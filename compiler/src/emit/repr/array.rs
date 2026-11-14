//! The arrays of the prelude. Each type of array has its own array type,
//! with each item in the form of a local of the rep of the items:
//!
//! ```wasm
//! (type $Items (array (mut i32)))
//! ```
//!
//! A literal of no items is the array of no items of its type, which the
//! module makes once.

use wasm_encoder::{ConstExpr, HeapType, Instruction, StorageType, ValType};

use super::super::module::{Composite, FuncState, GlobalIdx, LocalIdx, ModuleState, TypeIdx};

/// The most items of a literal that one `array.new_fixed` takes. A longer
/// literal sets each item of a new array, which keeps the items off the
/// stack and below the limit of 10000 of V8.
const FIXED_MAX: usize = 64;

pub(super) struct ArrayRepr {
    /// The type of each type of array, by its id.
    types: Vec<ArrayType>,
}

struct ArrayType {
    array: TypeIdx,
    /// The array of no items.
    empty: GlobalIdx,
}

impl ArrayRepr {
    /// The type of an array whose items have the local type `item`.
    pub fn composite(item: ValType) -> Composite {
        Composite::Array(StorageType::Val(item), true)
    }

    /// The arrays whose types are `types`, from the rec group of
    /// [`Self::composite`].
    pub fn new(m: &mut ModuleState, types: &[TypeIdx]) -> Self {
        let types = types
            .iter()
            .map(|&array| {
                let init = ConstExpr::extended([Instruction::ArrayNewFixed {
                    array_type_index: array.index(),
                    array_size: 0,
                }]);
                ArrayType {
                    array,
                    empty: m.global(array.ref_to(false), &init),
                }
            })
            .collect();
        ArrayRepr { types }
    }

    fn types(&self, array: usize) -> &ArrayType {
        self.types
            .get(array)
            .expect("a type of array of the program")
    }

    /// The type of the array `array`.
    pub fn array_type(&self, array: usize) -> TypeIdx {
        self.types(array).array
    }

    /// Starts a literal of the type `array` with `count` items. The code
    /// then pushes each item between [`Self::before_item`] and
    /// [`Self::after_item`], and ends with [`Self::end_literal`]. A literal
    /// with more than [`FIXED_MAX`] items fills an array that the returned
    /// local holds.
    pub fn begin_literal(&self, f: &mut FuncState, array: usize, count: usize) -> Option<LocalIdx> {
        if count <= FIXED_MAX {
            return None;
        }
        let ty = self.types(array).array;
        let local = f.temp(ty.ref_to(true));
        f.ins()
            .i32_const(len(count))
            .array_new_default(ty)
            .local_set(local);
        Some(local)
    }

    /// Comes before the item at `index` of a literal.
    pub fn before_item(&self, f: &mut FuncState, local: Option<LocalIdx>, index: usize) {
        if let Some(local) = local {
            f.ins().local_get(local).i32_const(len(index));
        }
    }

    /// Comes after an item of a literal of the type `array`.
    pub fn after_item(&self, f: &mut FuncState, array: usize, local: Option<LocalIdx>) {
        if local.is_some() {
            f.ins().array_set(self.types(array).array);
        }
    }

    /// Pushes the array of a literal of the type `array` with `count`
    /// items.
    pub fn end_literal(
        &self,
        f: &mut FuncState,
        array: usize,
        count: usize,
        local: Option<LocalIdx>,
    ) {
        let types = self.types(array);
        match local {
            Some(local) => {
                f.ins().local_get(local).ref_as_non_null();
                f.free(local);
            }
            None if count == 0 => {
                f.ins().global_get(types.empty);
            }
            None => {
                f.ins()
                    .array_new_fixed(types.array, u32::try_from(count).expect("at most 64 items"));
            }
        }
    }

    /// Pops an array of the type `array`, an `i32` start and an `i32`
    /// count, and sets that many items from the start to `null`, the null
    /// of the items when they are references. Items without a reference
    /// stay as they are.
    pub fn clear(&self, f: &mut FuncState, array: usize, null: Option<HeapType>) {
        let Some(null) = null else {
            f.ins().drop().drop().drop();
            return;
        };
        let count = f.temp(ValType::I32);
        f.ins()
            .local_set(count)
            .ref_null(null)
            .local_get(count)
            .array_fill(self.types(array).array);
        f.free(count);
    }

    /// Pops an array of the type `array` and an `i32` index, and pushes the
    /// item at the index.
    pub fn item(&self, f: &mut FuncState, array: usize) {
        f.ins().array_get(self.types(array).array);
    }

    /// Pops an array of the type `array`, an `i32` index and an item, and
    /// sets the item at the index.
    pub fn set_item(&self, f: &mut FuncState, array: usize) {
        f.ins().array_set(self.types(array).array);
    }

    /// Pops an array of the type `array`, an `i32` index, another array of
    /// the type, an `i32` index and an `i32` count, and copies that many
    /// items of the second array to the first.
    pub fn copy_items(&self, f: &mut FuncState, array: usize) {
        let ty = self.types(array).array;
        f.ins().array_copy(ty, ty);
    }
}

/// `count` as an `i32`, which an array of the program never passes.
fn len(count: usize) -> i32 {
    i32::try_from(count).expect("fewer than 2^31 items")
}
