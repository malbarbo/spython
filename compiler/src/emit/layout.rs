//! Where each class and each type of list of the program goes in the rec
//! group of the module, and what the struct of a class holds.

use wasm_encoder::{StorageType, ValType};

use super::module::{Composite, TypeIdx};
use crate::ir::{ClassId, Rep, UserClass};

/// The types of the rec group, from `first`. The struct of each class comes
/// first, each after the struct of its base, then the exact type of each
/// class with subclasses, then the struct and the array of each type of
/// list, then the struct of a value and the type of the code of each type
/// of function, then the struct of the values of each function that
/// captures variables, then the struct of a cell of each rep, and then the
/// node of the stack of the objects whose `repr` runs.
///
/// A struct of a class with subclasses is not final, and a `ref.test` on
/// such a type costs more than one on a final type. The exact type of the
/// class is a final subtype with the same fields, which `struct.new`
/// makes and each test names.
pub(crate) struct Layout {
    first: TypeIdx,
    classes: Vec<Shape>,
    /// The number of classes with subclasses.
    exacts: usize,
    lists: usize,
    funcs: usize,
    closures: usize,
    cells: usize,
}

/// What the layout knows of a class.
struct Shape {
    base: Option<ClassId>,
    /// Whether the fields start with no value.
    unset: bool,
    /// The place of the exact type among the exact types, for a class with
    /// subclasses.
    exact: Option<usize>,
}

impl Layout {
    pub fn new(
        first: TypeIdx,
        classes: &[UserClass],
        (lists, funcs, closures, cells): (usize, usize, usize, usize),
    ) -> Self {
        let mut exacts = 0;
        let classes = classes
            .iter()
            .enumerate()
            .map(|(i, class)| {
                let open = classes
                    .iter()
                    .any(|other| other.base.is_some_and(|base| base.index() == i));
                let exact = open.then(|| {
                    exacts += 1;
                    exacts - 1
                });
                Shape {
                    base: class.base,
                    unset: class.unset,
                    exact,
                }
            })
            .collect();
        Layout {
            first,
            classes,
            exacts,
            lists,
            funcs,
            closures,
            cells,
        }
    }

    /// The number of classes of the program.
    pub fn classes(&self) -> usize {
        self.classes.len()
    }

    /// The number of types of the classes, with the exact ones.
    pub fn class_types(&self) -> usize {
        self.classes() + self.exacts
    }

    fn shape(&self, class: ClassId) -> &Shape {
        self.classes
            .get(class.index())
            .expect("a class of the program")
    }

    /// Returns `true` if a field of a reference of `class` is nullable,
    /// because an object starts with no value in its fields, `false`
    /// otherwise.
    pub fn unset(&self, class: ClassId) -> bool {
        self.shape(class).unset
    }

    /// The base of `class`, and `None` for a class without one.
    pub fn base(&self, class: ClassId) -> Option<ClassId> {
        self.shape(class).base
    }

    /// Returns `true` if `class` is `ancestor` or descends from it, `false`
    /// otherwise.
    pub fn descends(&self, class: ClassId, ancestor: ClassId) -> bool {
        std::iter::successors(Some(class), |class| self.base(*class)).any(|c| c == ancestor)
    }

    /// The struct of an object of `class`, the static type of a value of
    /// the class.
    pub fn class(&self, class: ClassId) -> TypeIdx {
        assert!(class.index() < self.classes(), "a class of the program");
        self.first.plus(class.index())
    }

    /// The final type of an object whose class is exactly `class`.
    pub fn exact(&self, class: ClassId) -> TypeIdx {
        match self.shape(class).exact {
            Some(exact) => self.first.plus(self.classes() + exact),
            None => self.class(class),
        }
    }

    /// Returns `true` if `class` has subclasses, `false` otherwise.
    pub fn is_open(&self, class: ClassId) -> bool {
        self.shape(class).exact.is_some()
    }

    /// The struct and the array of the type of list at `list`.
    pub fn list(&self, list: usize) -> (TypeIdx, TypeIdx) {
        assert!(list < self.lists, "a type of list of the program");
        let ty = self.first.plus(self.class_types() + 2 * list);
        (ty, ty.plus(1))
    }

    /// The struct of a value of the type of function at `func`, and the
    /// type of its code, which takes the value before the parameters.
    pub fn func(&self, func: usize) -> (TypeIdx, TypeIdx) {
        assert!(func < self.funcs, "a type of function of the program");
        let ty = self
            .first
            .plus(self.class_types() + 2 * self.lists + 2 * func);
        (ty, ty.plus(1))
    }

    /// The struct of a value of the function at `closure` among the
    /// functions that capture variables, a final subtype of the struct of
    /// its type of function.
    pub fn closure(&self, closure: usize) -> TypeIdx {
        assert!(closure < self.closures, "a function that captures");
        self.first
            .plus(self.class_types() + 2 * self.lists + 2 * self.funcs + closure)
    }

    /// The struct of a cell of the rep at `cell` among the reps of cells.
    pub fn cell(&self, cell: usize) -> TypeIdx {
        assert!(cell < self.cells, "a rep of a cell");
        self.first
            .plus(self.class_types() + 2 * self.lists + 2 * self.funcs + self.closures + cell)
    }

    /// The node of the stack of the objects whose `repr` runs.
    pub fn repr_node(&self) -> TypeIdx {
        self.first
            .plus(self.class_types() + 2 * self.lists + 2 * self.funcs + self.closures + self.cells)
    }

    /// The struct of an object of `class`, with a field of the type that
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
