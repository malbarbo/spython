//! Where each class and each type of list of the program goes in the rec
//! group of the module, and what the struct of a class holds.

use wasm_encoder::{StorageType, ValType};

use super::module::{Composite, TypeIdx};
use crate::ir::{ClassId, Rep, UserClass};

/// The types of the rec group, from `first`. The struct of each class comes
/// first, then the struct and the array of each type of list, and then the
/// node of the stack of the objects whose `repr` runs.
pub(crate) struct Layout {
    first: TypeIdx,
    classes: usize,
    lists: usize,
}

impl Layout {
    pub fn new(first: TypeIdx, classes: usize, lists: usize) -> Self {
        Layout {
            first,
            classes,
            lists,
        }
    }

    /// The number of classes of the program.
    pub fn classes(&self) -> usize {
        self.classes
    }

    /// The struct of an object of `class`.
    pub fn class(&self, class: ClassId) -> TypeIdx {
        assert!(class.index() < self.classes, "a class of the program");
        self.first.plus(class.index())
    }

    /// The struct and the array of the type of list at `list`.
    pub fn list(&self, list: usize) -> (TypeIdx, TypeIdx) {
        assert!(list < self.lists, "a type of list of the program");
        let ty = self.first.plus(self.classes + 2 * list);
        (ty, ty.plus(1))
    }

    /// The node of the stack of the objects whose `repr` runs.
    pub fn repr_node(&self) -> TypeIdx {
        self.first.plus(self.classes + 2 * self.lists)
    }

    /// The struct of an object of `class`, with a field of the type
    /// `value_type` gives for the rep of each field.
    pub fn class_struct(class: &UserClass, value_type: impl Fn(Rep) -> ValType) -> Composite {
        Composite::Struct(
            class
                .fields
                .iter()
                .map(|&rep| (StorageType::Val(value_type(rep)), class.mutable))
                .collect(),
        )
    }
}
