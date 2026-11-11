//! The lists. Each type of list has its own struct and its own array, with
//! each item in the form of a local of the rep of the items:
//!
//! ```wasm
//! (type $Items (array (mut i32)))
//! (type $List (struct (field $len (mut i32)) (field $items (mut (ref $Items)))))
//! ```
//!
//! The array may have room for more items than the list has. An empty list
//! starts with the array of no items of its type, which the module makes
//! once.

use wasm_encoder::{BlockType, ConstExpr, Instruction, StorageType, ValType};

use super::super::module::{Composite, FuncState, GlobalIdx, LocalIdx, ModuleState, TypeIdx};

/// The most items of a literal that one `array.new_fixed` takes. A longer
/// literal sets each item of a new array, which keeps the items off the
/// stack and below the limit of 10000 of V8.
const FIXED_MAX: usize = 64;

pub(super) struct ListRepr {
    /// The struct and the array of each type of list, by its id.
    types: Vec<ListTypes>,
}

struct ListTypes {
    list: TypeIdx,
    items: TypeIdx,
    /// The array of no items, which every empty list starts with.
    empty: GlobalIdx,
}

/// The field of the length of a list.
const LEN: u32 = 0;
/// The field of the array of the items of a list.
const ITEMS: u32 = 1;

impl ListRepr {
    /// The struct and the array of a type of list whose items have the
    /// local type `item`, as two types of a rec group whose first type is
    /// `first`.
    pub fn composites(first: TypeIdx, item: ValType) -> [Composite; 2] {
        let items = first.plus(1);
        [
            Composite::Struct(vec![
                (StorageType::Val(ValType::I32), true),
                (StorageType::Val(items.ref_to(false)), true),
            ]),
            Composite::Array(StorageType::Val(item), true),
        ]
    }

    /// The lists whose struct and array are each pair of `types`, from the
    /// rec group of [`Self::composites`].
    pub fn new(m: &mut ModuleState, types: &[(TypeIdx, TypeIdx)]) -> Self {
        let types = types
            .iter()
            .map(|&(list, items)| {
                let init = ConstExpr::extended([Instruction::ArrayNewFixed {
                    array_type_index: items.index(),
                    array_size: 0,
                }]);
                ListTypes {
                    list,
                    items,
                    empty: m.global(items.ref_to(false), &init),
                }
            })
            .collect();
        ListRepr { types }
    }

    /// The lists of a module that has no type of list yet.
    pub fn empty() -> Self {
        ListRepr { types: Vec::new() }
    }

    fn types(&self, list: usize) -> &ListTypes {
        self.types.get(list).expect("a type of list of the program")
    }

    /// The struct of a list of the type `list`.
    pub fn list_type(&self, list: usize) -> TypeIdx {
        self.types(list).list
    }

    /// Starts a literal of the type `list` with `count` items. The code
    /// then pushes each item between [`Self::before_item`] and
    /// [`Self::after_item`], and ends with [`Self::end_literal`]. A literal
    /// with more than [`FIXED_MAX`] items fills an array that the returned
    /// local holds.
    pub fn begin_literal(&self, f: &mut FuncState, list: usize, count: usize) -> Option<LocalIdx> {
        f.ins().i32_const(len(count));
        if count <= FIXED_MAX {
            return None;
        }
        let items = self.types(list).items;
        let array = f.temp(items.ref_to(true));
        f.ins()
            .i32_const(len(count))
            .array_new_default(items)
            .local_set(array);
        Some(array)
    }

    /// Comes before the item at `index` of a literal.
    pub fn before_item(&self, f: &mut FuncState, array: Option<LocalIdx>, index: usize) {
        if let Some(array) = array {
            f.ins().local_get(array).i32_const(len(index));
        }
    }

    /// Comes after an item of a literal of the type `list`.
    pub fn after_item(&self, f: &mut FuncState, list: usize, array: Option<LocalIdx>) {
        if array.is_some() {
            f.ins().array_set(self.types(list).items);
        }
    }

    /// Pushes the list of a literal of the type `list` with `count` items.
    pub fn end_literal(
        &self,
        f: &mut FuncState,
        list: usize,
        count: usize,
        array: Option<LocalIdx>,
    ) {
        let types = self.types(list);
        match array {
            Some(array) => {
                f.ins().local_get(array).ref_as_non_null();
                f.free(array);
            }
            None if count == 0 => {
                f.ins().global_get(types.empty);
            }
            None => {
                f.ins()
                    .array_new_fixed(types.items, u32::try_from(count).expect("at most 64 items"));
            }
        }
        f.ins().struct_new(types.list);
    }

    /// Pops a list of the type `list` and pushes the length of its array as
    /// an `i32`.
    pub fn capacity(&self, f: &mut FuncState, list: usize) {
        let types = self.types(list);
        f.ins().struct_get(types.list, ITEMS).array_len();
    }

    /// Pops a list of the type `list` and an `i32` capacity, not below the
    /// length of the list, and gives the list a new array of that length
    /// with its items.
    pub fn reserve(&self, f: &mut FuncState, list: usize) {
        let types = self.types(list);
        let capacity = f.temp(ValType::I32);
        let value = f.temp(types.list.ref_to(true));
        let array = f.temp(types.items.ref_to(true));
        f.ins()
            .local_set(capacity)
            .local_set(value)
            .local_get(capacity)
            .array_new_default(types.items)
            .local_tee(array)
            .i32_const(0)
            .local_get(value)
            .struct_get(types.list, ITEMS)
            .i32_const(0)
            .local_get(value)
            .struct_get(types.list, LEN)
            .array_copy(types.items, types.items)
            .local_get(value)
            .local_get(array)
            .ref_as_non_null()
            .struct_set(types.list, ITEMS);
        f.free(capacity);
        f.free(value);
        f.free(array);
    }

    /// Pops a list of the type `list` and an `i32`, and sets the length of
    /// the list to it.
    pub fn set_len(&self, f: &mut FuncState, list: usize) {
        f.ins().struct_set(self.types(list).list, LEN);
    }

    /// Pops a list of the type `list` and pushes the array of its items.
    pub fn items(&self, f: &mut FuncState, list: usize) {
        f.ins().struct_get(self.types(list).list, ITEMS);
    }

    /// Pops the array of the items of a list of the type `list` and an `i32`
    /// index, and pushes the item of the array at the index.
    pub fn item(&self, f: &mut FuncState, list: usize) {
        f.ins().array_get(self.types(list).items);
    }

    /// Pops the array of the items of a list of the type `list`, an `i32`
    /// index and an item, and sets the item of the array at the index.
    pub fn set_item(&self, f: &mut FuncState, list: usize) {
        f.ins().array_set(self.types(list).items);
    }

    /// Pops the array of the items of a list of the type `list`, an `i32`
    /// index, another array of the type, an `i32` index and an `i32`
    /// count, and copies that many items of the second array to the first.
    pub fn copy_items(&self, f: &mut FuncState, list: usize) {
        let items = self.types(list).items;
        f.ins().array_copy(items, items);
    }

    /// Pops a list of the type `list` and pushes its number of items as an
    /// `i32`.
    pub fn len(&self, f: &mut FuncState, list: usize) {
        f.ins().struct_get(self.types(list).list, LEN);
    }

    /// Pops a list of the type `list`, which may be null, and pushes the
    /// `bool` of whether it is a list with items.
    pub fn truth(&self, f: &mut FuncState, list: usize) {
        let ty = self.types(list).list;
        let value = f.temp(ty.ref_to(true));
        f.ins().local_tee(value).ref_is_null();
        f.if_(BlockType::Result(ValType::I32));
        f.ins().i32_const(0);
        f.else_();
        f.ins()
            .local_get(value)
            .struct_get(ty, LEN)
            .i32_const(0)
            .i32_ne();
        f.end();
        f.free(value);
    }
}

/// `count` as an `i32`, which a list of the program never passes.
fn len(count: usize) -> i32 {
    i32::try_from(count).expect("fewer than 2^31 items")
}
