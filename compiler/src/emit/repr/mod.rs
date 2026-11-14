//! How each value is laid out in wasm. No other module knows it.

mod float;
mod int;
mod list;
pub(super) mod str;
mod uniform;

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, HeapType, StorageType, ValType};

use super::layout::Layout;
use super::module::{
    Composite, FuncIdx, FuncState, GlobalIdx, Label, LocalIdx, ModuleState, TypeIdx,
};
use super::runtime::exc::Exceptions;
use super::runtime::host::{Capture, Host};
use crate::cpython::{
    Class, LIST_ASSIGNMENT_OUT_OF_RANGE, LIST_INDEX_OUT_OF_RANGE, MATH_DOMAIN_ERROR, MEMORY_ERROR,
    Message, STR_INDEX_OUT_OF_RANGE,
};
use crate::ir::{
    ArithOp, BitOp, Capture as CaptureOf, Case, Choice, ClassId, CmpOp, CodePointSet, DivOp,
    FloatFunc2, FloatOp, FuncType, FuncTypeId, ListId, ListType, Literal, MathFunc, MathFunc2, Rep,
    SiteFrame, UserClass,
};
use float::FloatRepr;
use int::IntRepr;
use list::ListRepr;
use str::StrRepr;
use uniform::UniformRepr;

/// The `hash` of `None`, a constant since CPython 3.12.
const NONE_HASH: i64 = 4238894112;
/// The primes of the xxHash of the `hash` of a tuple.
const XXPRIME_1: u64 = 11400714785074694791;
const XXPRIME_2: u64 = 14029467366897019727;
const XXPRIME_5: u64 = 2870177450012600261;

pub(crate) struct Reprs {
    int: IntRepr,
    float: FloatRepr,
    str: StrRepr,
    uniform: UniformRepr,
    list: ListRepr,
    /// `(func (param (ref null eq)))`, which writes a value of a union. A
    /// module that writes none goes without it.
    write_uniform: Option<FuncIdx>,
    /// `(func (param (ref null eq) (ref null eq)) (result i32))`, which
    /// compares two values of a union, as [`Self::add_compare_uniform`]
    /// says.
    compare_uniform: Option<FuncIdx>,
    /// The function of [`Self::uniform_eq`] for each list of dataclasses
    /// and their functions.
    uniform_eqs: HashMap<Vec<(ClassId, FuncIdx)>, FuncIdx>,
    /// `(func (param (ref null eq)) (result (ref $Str)))`, which gives the
    /// `repr` of a value of a union. A module that gives none goes without
    /// it.
    repr_uniform: Option<FuncIdx>,
    /// `(func (param (ref null eq)) (result i32))`, which gives the truth
    /// of a value of a union, true for an object.
    truth_uniform: Option<FuncIdx>,
    /// `(func (param (ref null eq)) (result i64))`, which gives the `hash`
    /// of `None`, a number or a `str` of a union.
    hash_uniform: Option<FuncIdx>,
    /// `(func (param (ref null eq)) (result i32))`, which tells whether
    /// the `repr` of an object runs.
    in_repr: Option<FuncIdx>,
    exc: Exceptions,
    layout: Layout,
    /// The global of each member of each class, which `_start` fills.
    members: Vec<Vec<GlobalIdx>>,
    /// The function that gives the `str` of an object of each class, when
    /// the program writes one.
    class_strs: Vec<Option<FuncIdx>>,
    /// The function that gives the `repr` of an object of each class, and
    /// the one that gives the `repr` of a `str`, when the program gives the
    /// `repr` of a value of a union.
    class_reprs: Vec<Option<FuncIdx>>,
    str_repr: Option<FuncIdx>,
    /// The `__eq__` of each copy of a generic class of the prelude, which
    /// takes the object and a uniform value.
    copy_eqs: Vec<(ClassId, FuncIdx)>,
    /// The rep of the items of each type of list.
    list_items: Vec<Rep>,
    /// The function that gives the `repr` of a list of each type, when the
    /// program writes one.
    list_reprs: Vec<Option<FuncIdx>>,
    /// The struct of a node of the stack of the objects whose `repr` runs,
    /// with the object and the node under it, and the global of the top of
    /// the stack, when a class guards its `repr`.
    repr_stack: Option<(TypeIdx, GlobalIdx)>,
    /// The rep of each cell, in the order of their structs.
    cells: Vec<Rep>,
}

/// The types of the program that the module lays out.
#[derive(Clone, Copy)]
pub(crate) struct Types<'p> {
    pub classes: &'p [UserClass],
    pub lists: &'p [ListType],
    pub funcs: &'p [FuncType],
    /// The type of function and the captures of each function that
    /// captures variables.
    pub closures: &'p [(FuncTypeId, &'p [CaptureOf])],
    /// The rep of each cell.
    pub cells: &'p [Rep],
}

impl Reprs {
    /// The representations of a module with the types of `types`, which
    /// runs doctests when `tests` is `true`.
    pub fn new(m: &mut ModuleState, host: &Host, tests: bool, types: &Types) -> Self {
        let Types {
            classes,
            lists,
            funcs,
            closures,
            cells,
        } = *types;
        let mut str = StrRepr::ucs4(m, host);
        let exc = Exceptions::new(m, host, &mut str, tests);
        let int = IntRepr::i31(m, &mut str, &exc);
        let float = FloatRepr::f64(host);
        // A field of a class may hold an object of any class, its own too,
        // or a list, and an item of a list may be an object or another list,
        // so the types refer to each other by the indices that they will have
        // in their rec group.
        let layout = Layout::new(
            m.next_type_index(),
            classes,
            (lists.len(), funcs.len(), closures.len(), cells.len()),
        );
        let list_types: Vec<(TypeIdx, TypeIdx)> =
            (0..lists.len()).map(|i| layout.list(i)).collect();
        let mut reprs = Reprs {
            int,
            float,
            str,
            uniform: UniformRepr::new(),
            list: ListRepr::new(m, &list_types),
            write_uniform: None,
            repr_uniform: None,
            truth_uniform: None,
            hash_uniform: None,
            compare_uniform: None,
            uniform_eqs: HashMap::new(),
            in_repr: None,
            exc,
            layout,
            members: Vec::new(),
            class_strs: Vec::new(),
            class_reprs: Vec::new(),
            str_repr: None,
            copy_eqs: Vec::new(),
            list_items: lists.iter().map(|list| list.item).collect(),
            list_reprs: Vec::new(),
            repr_stack: None,
            cells: cells.to_vec(),
        };
        let structs: Vec<Composite> = classes
            .iter()
            .map(|class| {
                Layout::class_struct(class, |rep| {
                    let ty = if class.unset {
                        reprs.local_type(rep)
                    } else {
                        reprs.value_type(rep)
                    };
                    ty.expect("a field has a value")
                })
            })
            .collect();
        let mut composites = Vec::new();
        for i in 0..classes.len() {
            let class = ClassId::new(i);
            let base = reprs
                .layout
                .base(class)
                .map(|base| reprs.layout.class(base));
            composites.push((i, base, !reprs.layout.is_open(class)));
        }
        for i in 0..classes.len() {
            let class = ClassId::new(i);
            if reprs.layout.is_open(class) {
                composites.push((i, Some(reprs.layout.class(class)), true));
            }
        }
        let mut composites: Vec<(&Composite, Option<TypeIdx>, bool)> = composites
            .into_iter()
            .map(|(i, base, is_final)| (structs.get(i).expect("a class"), base, is_final))
            .collect();
        let mut others = Vec::new();
        for (list, &(_, items)) in lists.iter().zip(&list_types) {
            let item = reprs.local_type(list.item).expect("an item has a value");
            others.extend(
                ListRepr::composites(items, item)
                    .into_iter()
                    .map(|composite| (composite, None, true)),
            );
        }
        // The struct of a value of a type of function is open, for the
        // values whose code captures variables.
        for (i, func) in funcs.iter().enumerate() {
            let (value, code) = reprs.layout.func(i);
            let params = std::iter::once(value.ref_to(true))
                .chain(
                    func.params
                        .iter()
                        .map(|rep| reprs.local_type(*rep).expect("a parameter has a value")),
                )
                .collect();
            let results = reprs.value_type(func.ret).into_iter().collect();
            others.push((
                Composite::Struct(vec![(StorageType::Val(code.ref_to(false)), false)]),
                None,
                false,
            ));
            others.push((Composite::Func(params, results), None, true));
        }
        for (ty, captures) in closures {
            let (value, code) = reprs.layout.func(ty.index());
            let fields = std::iter::once((StorageType::Val(code.ref_to(false)), false))
                .chain(captures.iter().map(|capture| {
                    let ty = if capture.cell {
                        reprs.cell_type(capture.rep).ref_to(true)
                    } else {
                        reprs
                            .local_type(capture.rep)
                            .expect("a capture has a value")
                    };
                    (StorageType::Val(ty), false)
                }))
                .collect();
            others.push((Composite::Struct(fields), Some(value), true));
        }
        // A cell holds its value and whether the variable has one.
        for rep in cells {
            let value = reprs.local_type(*rep).expect("a cell has a value");
            others.push((
                Composite::Struct(vec![
                    (StorageType::Val(value), true),
                    (StorageType::Val(ValType::I32), true),
                ]),
                None,
                true,
            ));
        }
        let node = reprs.layout.repr_node();
        let guarded = classes.iter().any(|class| class.repr_guard);
        if guarded {
            let object = reprs.uniform.val_type();
            others.push((
                Composite::Struct(vec![
                    (StorageType::Val(object), false),
                    (StorageType::Val(node.ref_to(true)), false),
                ]),
                None,
                true,
            ));
        }
        composites.extend(
            others
                .iter()
                .map(|(composite, base, is_final)| (composite, *base, *is_final)),
        );
        let mut group = m.rec_group(&composites);
        if guarded {
            let top = m.global(node.ref_to(true), &ConstExpr::ref_null(node.heap_type()));
            reprs.repr_stack = Some((group.pop().expect("the node of the stack"), top));
        }
        let listed: Vec<TypeIdx> = list_types.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let mut group_lists = group.split_off(reprs.layout.class_types());
        group_lists.truncate(listed.len());
        debug_assert_eq!(group_lists, listed, "the lists in their rec group");
        group.truncate(classes.len());
        debug_assert_eq!(
            group,
            (0..classes.len())
                .map(|i| reprs.layout.class(ClassId::new(i)))
                .collect::<Vec<_>>(),
            "the classes in their rec group"
        );
        reprs.members = classes
            .iter()
            .zip(group)
            .map(|(class, ty)| {
                class
                    .members
                    .iter()
                    .map(|fields| reprs.member_global(m, ty, fields))
                    .collect()
            })
            .collect();
        reprs
    }

    /// The global of an object of the struct `ty` with the literals of
    /// `fields`, which `_start` makes. A `str` is not a constant
    /// expression, so the global is mutable and starts as null.
    fn member_global(&mut self, m: &mut ModuleState, ty: TypeIdx, fields: &[Literal]) -> GlobalIdx {
        let global = m.global(ty.ref_to(true), &ConstExpr::ref_null(ty.heap_type()));
        let texts: Vec<Option<GlobalIdx>> = fields
            .iter()
            .map(|field| match field {
                Literal::Str(text) => Some(self.str.literal_global(m, text)),
                Literal::Int(_) => None,
            })
            .collect();
        let mut ins = m.init_value();
        for (field, text) in fields.iter().zip(texts) {
            match (field, text) {
                (Literal::Int(value), _) => self.int.init_literal(&mut ins, *value),
                (Literal::Str(_), text) => {
                    let text = text.expect("the global of the text");
                    ins.global_get(text).ref_as_non_null();
                }
            }
        }
        ins.struct_new(ty).global_set(global);
        global
    }

    /// Takes the function that gives the `str` of an object of each class,
    /// for the classes whose objects the program writes.
    pub fn set_class_strs(&mut self, strs: Vec<Option<FuncIdx>>) {
        self.class_strs = strs;
    }

    /// Takes the function that gives the `repr` of an object of each class,
    /// and the one that gives the `repr` of a `str`, for a program that
    /// gives the `repr` of a value of a union.
    pub fn set_reprs(&mut self, classes: Vec<Option<FuncIdx>>, str: Option<FuncIdx>) {
        self.class_reprs = classes;
        self.str_repr = str;
    }

    /// Takes the `__eq__` of each copy of a generic class of the prelude
    /// whose objects the program makes.
    pub fn set_copy_eqs(&mut self, eqs: Vec<(ClassId, FuncIdx)>) {
        self.copy_eqs = eqs;
    }

    /// Takes the function that gives the `repr` of a list of each type, for
    /// the types whose lists the program writes.
    pub fn set_list_reprs(&mut self, reprs: Vec<Option<FuncIdx>>) {
        self.list_reprs = reprs;
    }

    /// Pops a value of a union and pushes its `repr`, which may raise.
    pub fn uniform_repr(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let func = match self.repr_uniform {
            Some(func) => func,
            None => {
                let func = self.add_repr_uniform(m);
                self.repr_uniform = Some(func);
                func
            }
        };
        f.ins().call(func);
    }

    /// Defines the function that gives the `repr` of a value of a union,
    /// after the test of its type.
    fn add_repr_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        let results = [self.str.val_type(false)];
        let classes = (0..self.layout.classes()).map(ClassId::new).collect();
        self.add_uniform_dispatch(m, &results, classes, |reprs, m, f, rep| match rep {
            Rep::None => reprs.str.literal(m, f, "None"),
            Rep::Str => {
                f.ins().call(reprs.str_repr.expect("the repr of a str"));
            }
            Rep::Object(class) => {
                let repr = reprs.class_reprs.get(class.index()).copied().flatten();
                f.ins().call(repr.expect("the repr of each class"));
            }
            rep => reprs.str_of(m, f, rep),
        })
    }

    /// Defines a function that takes a value of a union, tests its type,
    /// and runs `on` with the rep of its value, which `on` finds on the
    /// stack in the form of that rep. `None` has no value on the stack. A
    /// value of a class out of `classes` stops the program.
    fn add_uniform_dispatch(
        &mut self,
        m: &mut ModuleState,
        results: &[ValType],
        classes: Vec<ClassId>,
        on: fn(&mut Self, &mut ModuleState, &mut FuncState, Rep),
    ) -> FuncIdx {
        let ty = m.func_type(&[self.uniform.val_type()], results);
        let mut f = FuncState::new(1);
        let value = f.param(0);
        f.ins().local_get(value).ref_is_null();
        f.if_(BlockType::Empty);
        on(self, m, &mut f, Rep::None);
        f.ins().return_();
        f.end();
        let boxes = self.uniform.boxes(m).into_iter();
        let str = self.str.type_index().heap_type();
        // Each test names the exact type of a class, so an object of a
        // subclass goes to its own class.
        let objects = classes
            .into_iter()
            .map(|class| (Rep::Object(class), self.layout.exact(class).heap_type()));
        let tests: Vec<(Rep, HeapType)> = boxes.chain([(Rep::Str, str)]).chain(objects).collect();
        for (rep, ty) in tests {
            f.ins().local_get(value).ref_test(ty);
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            match rep {
                Rep::Object(_) => {
                    f.ins().ref_cast(ty, false);
                }
                rep => self.narrow(m, &mut f, Rep::Uniform, rep),
            }
            on(self, m, &mut f, rep);
            f.ins().return_();
            f.end();
        }
        f.ins().unreachable();
        m.add_func(ty, f.finish())
    }

    /// Pops an object of `class` and pushes its field at `field`. A field
    /// has the type of a value of its rep.
    pub fn field(&self, f: &mut FuncState, class: ClassId, field: u32, rep: Rep) {
        f.ins().struct_get(self.class(class), field);
        if self.layout.unset(class) {
            non_null(f, rep);
        }
    }

    /// Pops a value for each field of `class` and pushes a new object of
    /// `class` with them. A value of each rep is a value of its field.
    pub fn new_object(&self, f: &mut FuncState, class: ClassId) {
        f.ins().struct_new(self.layout.exact(class));
    }

    /// Pushes a new object of `class` whose fields have no value.
    pub fn empty_object(&self, f: &mut FuncState, class: ClassId) {
        f.ins().struct_new_default(self.layout.exact(class));
    }

    /// Pops an object and pushes the `bool` of whether its class is
    /// exactly `class`.
    pub fn is_exact(&self, f: &mut FuncState, class: ClassId) {
        f.ins().ref_test(self.layout.exact(class).heap_type());
    }

    /// Pops an object whose class is exactly `class` and pushes it as a
    /// reference to the exact type of the class.
    pub fn as_exact(&self, f: &mut FuncState, class: ClassId) {
        f.ins()
            .ref_cast(self.layout.exact(class).heap_type(), false);
    }

    /// Pops an object of `class` and a value of the field at `field`, and
    /// sets the field to the value.
    pub fn set_field(&self, f: &mut FuncState, class: ClassId, field: u32) {
        f.ins().struct_set(self.class(class), field);
    }

    /// The node and the top of the stack of the objects whose `repr` runs.
    fn repr_stack(&self) -> (TypeIdx, GlobalIdx) {
        self.repr_stack.expect("a class that guards its repr")
    }

    /// Pops an object and pushes whether its `repr` runs, as an `i32`.
    pub fn in_repr(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let func = match self.in_repr {
            Some(func) => func,
            None => {
                let func = self.add_in_repr(m);
                self.in_repr = Some(func);
                func
            }
        };
        f.ins().call(func);
    }

    /// Defines the function that looks for its object in the stack of the
    /// objects whose `repr` runs.
    fn add_in_repr(&self, m: &mut ModuleState) -> FuncIdx {
        let (node, top) = self.repr_stack();
        let ty = m.func_type(&[self.uniform.val_type()], &[ValType::I32]);
        let mut f = FuncState::new(1);
        let object = f.param(0);
        let next = f.temp(node.ref_to(true));
        f.ins().global_get(top).local_set(next);
        let nodes = f.loop_(BlockType::Empty);
        f.ins().local_get(next).ref_is_null();
        f.if_(BlockType::Empty);
        f.ins().i32_const(0).return_();
        f.end();
        f.ins()
            .local_get(next)
            .ref_as_non_null()
            .struct_get(node, 0)
            .local_get(object)
            .ref_eq();
        f.if_(BlockType::Empty);
        f.ins().i32_const(1).return_();
        f.end();
        f.ins()
            .local_get(next)
            .ref_as_non_null()
            .struct_get(node, 1)
            .local_set(next);
        f.br(nodes);
        f.end();
        f.ins().unreachable();
        m.add_func(ty, f.finish())
    }

    /// Pops an object and puts it on the top of the stack of the objects
    /// whose `repr` runs.
    pub fn push_repr(&self, f: &mut FuncState) {
        let (node, top) = self.repr_stack();
        f.ins().global_get(top).struct_new(node).global_set(top);
    }

    /// Takes the object on the top of the stack of the objects whose
    /// `repr` runs off the stack.
    pub fn pop_repr(&self, f: &mut FuncState) {
        let (node, top) = self.repr_stack();
        f.ins()
            .global_get(top)
            .ref_as_non_null()
            .struct_get(node, 1)
            .global_set(top);
    }

    /// Pushes the member at `member` of `class`.
    pub fn member(&self, f: &mut FuncState, class: ClassId, member: u32) {
        let global = self
            .members
            .get(class.index())
            .and_then(|members| members.get(member as usize))
            .expect("a member of the class");
        f.ins().global_get(*global).ref_as_non_null();
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
    pub fn end_module(&self, f: &mut FuncState, site: LocalIdx) {
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
    pub fn end_function(&self, f: &mut FuncState, site: LocalIdx) {
        self.exc.end_function(f, site);
    }

    /// Adds what the module needs once the program is emitted, with the
    /// text of the frame of each site.
    pub fn finish(&mut self, m: &mut ModuleState, sites: &[SiteFrame]) {
        self.exc.finish(m, &self.str, sites);
    }

    /// The type of a local that holds a value of `rep`, and `None` for a
    /// rep that has no value. A reference is nullable in a local, and
    /// [`Self::load`] reads it back.
    pub fn local_type(&self, rep: Rep) -> Option<ValType> {
        match rep {
            Rep::Int => Some(self.int.val_type()),
            Rep::Float => Some(self.float.val_type()),
            // `False` and `True` are the `i32` 0 and 1.
            Rep::Bool => Some(ValType::I32),
            Rep::Str | Rep::StrOrNone => Some(self.str.val_type(true)),
            Rep::Uniform => Some(self.uniform.val_type()),
            Rep::Object(class) | Rep::ObjectOrNone(class) => Some(self.class(class).ref_to(true)),
            Rep::List(list) | Rep::ListOrNone(list) => Some(self.list_type(list).ref_to(true)),
            Rep::Hash => Some(ValType::I64),
            Rep::Func(ty) => Some(self.func_value(ty).ref_to(true)),
            Rep::None => None,
        }
    }

    /// The struct of a cell of `rep`.
    pub fn cell_type(&self, rep: Rep) -> TypeIdx {
        let cell = self
            .cells
            .iter()
            .position(|other| *other == rep)
            .expect("a rep of a cell");
        self.layout.cell(cell)
    }

    /// The struct of a value of the function at `closure` among the
    /// functions that capture variables.
    pub fn closure_type(&self, closure: usize) -> TypeIdx {
        self.layout.closure(closure)
    }

    /// Pushes a value of `rep` from a field or a local that may hold null
    /// for a value that is never null.
    pub fn non_null(&self, f: &mut FuncState, rep: Rep) {
        non_null(f, rep);
    }

    /// The struct of a value of the type of function `ty`.
    pub fn func_value(&self, ty: FuncTypeId) -> TypeIdx {
        self.layout.func(ty.index()).0
    }

    /// The type of the code of a value of the type of function `ty`.
    pub fn func_code(&self, ty: FuncTypeId) -> TypeIdx {
        self.layout.func(ty.index()).1
    }

    /// The struct of a list of the type `list`.
    fn list_type(&self, list: ListId) -> TypeIdx {
        self.list.list_type(list.index())
    }

    /// The struct of an object of `class`.
    fn class(&self, class: ClassId) -> TypeIdx {
        self.layout.class(class)
    }

    /// The type of a value of `rep` on the stack, and `None` for a rep that
    /// has no value. An object may be null, which stands for a `None` that
    /// a field got after the test that narrows it.
    pub fn value_type(&self, rep: Rep) -> Option<ValType> {
        match rep {
            Rep::Str => Some(self.str.val_type(false)),
            Rep::List(list) => Some(self.list_type(list).ref_to(false)),
            _ => self.local_type(rep),
        }
    }

    /// Pushes the value of `rep` that `local` holds.
    pub fn load(&self, f: &mut FuncState, rep: Rep, local: LocalIdx) {
        f.ins().local_get(local);
        non_null(f, rep);
    }

    /// A global that holds a value of `rep`. Until the first `global.set`,
    /// it holds a value that no read sees.
    pub fn global(&self, m: &mut ModuleState, rep: Rep) -> GlobalIdx {
        let ty = self.local_type(rep).expect("a variable has a value");
        let init = match rep {
            Rep::Int => self.int.global_init(),
            Rep::Float => self.float.global_init(),
            Rep::Bool => ConstExpr::i32_const(0),
            Rep::Str | Rep::StrOrNone => ConstExpr::ref_null(self.str.type_index().heap_type()),
            Rep::Uniform => ConstExpr::ref_null(uniform::EQ),
            Rep::Object(class) | Rep::ObjectOrNone(class) => {
                ConstExpr::ref_null(self.class(class).heap_type())
            }
            Rep::List(list) | Rep::ListOrNone(list) => {
                ConstExpr::ref_null(self.list_type(list).heap_type())
            }
            Rep::Hash => ConstExpr::i64_const(0),
            Rep::Func(ty) => ConstExpr::ref_null(self.func_value(ty).heap_type()),
            Rep::None => unreachable!("a variable of rep None"),
        };
        m.global(ty, &init)
    }

    /// Pushes the value of `rep` that `global` holds.
    pub fn load_global(&self, f: &mut FuncState, rep: Rep, global: GlobalIdx) {
        f.ins().global_get(global);
        non_null(f, rep);
    }

    /// Pops a value of `from` and pushes it as a value of `to`, a rep that
    /// holds the values of `from`.
    pub fn widen(&mut self, m: &mut ModuleState, f: &mut FuncState, from: Rep, to: Rep) {
        match (from, to) {
            (Rep::Int, Rep::Uniform) => {
                self.int.as_i32(f);
                self.uniform.box_int(f);
            }
            (Rep::Float, Rep::Uniform) => self.uniform.box_float(m, f),
            (Rep::Bool, Rep::Uniform) => self.uniform.box_bool(m, f),
            (Rep::None, Rep::Uniform) => self.uniform.none(f),
            // A `$Str` is an `eq`, and null is `None` in both.
            (Rep::Str | Rep::StrOrNone, Rep::Uniform) => {}
            (Rep::Str, Rep::StrOrNone) => {}
            (Rep::None, Rep::StrOrNone) => {
                f.ins().ref_null(self.str.type_index().heap_type());
            }
            // An object is an `eq`, and null is `None`.
            (Rep::Object(_) | Rep::ObjectOrNone(_), Rep::Uniform) => {}
            // The struct of a class is a subtype of the struct of its base.
            (Rep::Object(a), Rep::Object(b) | Rep::ObjectOrNone(b))
            | (Rep::ObjectOrNone(a), Rep::ObjectOrNone(b))
                if self.layout.descends(a, b) => {}
            (Rep::None, Rep::ObjectOrNone(class)) => {
                f.ins().ref_null(self.class(class).heap_type());
            }
            // A list is an `eq`, and null is `None`.
            (Rep::List(_) | Rep::ListOrNone(_), Rep::Uniform) => {}
            (Rep::List(a), Rep::ListOrNone(b)) if a == b => {}
            (Rep::None, Rep::ListOrNone(list)) => {
                f.ins().ref_null(self.list_type(list).heap_type());
            }
            _ => unreachable!("a widening of {from:?} to {to:?}"),
        }
    }

    /// Pops a value of `from`, a union, and pushes it as a value of `to`,
    /// one of the types of the union, which the checker knows the value to
    /// have.
    pub fn narrow(&mut self, m: &mut ModuleState, f: &mut FuncState, from: Rep, to: Rep) {
        match (from, to) {
            (
                Rep::StrOrNone | Rep::Uniform | Rep::ObjectOrNone(_) | Rep::ListOrNone(_),
                Rep::None,
            ) => {
                f.ins().drop();
            }
            (Rep::ListOrNone(a), Rep::List(b)) if a == b => {
                f.ins().ref_as_non_null();
            }
            (Rep::Uniform, Rep::List(list) | Rep::ListOrNone(list)) => {
                f.ins().ref_cast(
                    self.list_type(list).heap_type(),
                    matches!(to, Rep::ListOrNone(_)),
                );
            }
            (Rep::StrOrNone, Rep::Str) => {
                f.ins().ref_as_non_null();
            }
            // An object may be null.
            (Rep::ObjectOrNone(a), Rep::Object(b)) if a == b => {}
            (Rep::Object(a) | Rep::ObjectOrNone(a), Rep::Object(b) | Rep::ObjectOrNone(b))
                if self.layout.descends(b, a) =>
            {
                self.cast_class(f, b);
            }
            (Rep::Uniform, Rep::Object(class) | Rep::ObjectOrNone(class)) => {
                self.cast_class(f, class);
            }
            (Rep::Uniform, Rep::Str | Rep::StrOrNone) => {
                f.ins()
                    .ref_cast(self.str.type_index().heap_type(), to == Rep::StrOrNone);
            }
            (Rep::Uniform, Rep::Int) => {
                self.uniform.unbox_int(f);
                self.int.small_as_int(f);
            }
            (Rep::Uniform, Rep::Float) => self.uniform.unbox_float(m, f),
            (Rep::Uniform, Rep::Bool) => self.uniform.unbox_bool(m, f),
            _ => unreachable!("a narrowing of {from:?} to {to:?}"),
        }
    }

    /// Pops a value of `from`, a union, and pushes the `bool` of whether it
    /// is a value of `to`, one of the types of the union.
    pub fn has(&mut self, m: &mut ModuleState, f: &mut FuncState, from: Rep, to: Rep) {
        let str = self.str.type_index().heap_type();
        match (from, to) {
            (Rep::StrOrNone, Rep::Str) | (Rep::ListOrNone(_), Rep::List(_)) => {
                f.ins().ref_is_null().i32_eqz();
            }
            (Rep::Uniform, Rep::List(list)) => {
                f.ins().ref_test(self.list_type(list).heap_type());
            }
            (Rep::Uniform, Rep::ListOrNone(list)) => {
                f.ins().ref_test_nullable(self.list_type(list).heap_type());
            }
            (Rep::Uniform, Rep::Int | Rep::Float | Rep::Bool) => {
                let ty = self.uniform.box_type(m, to);
                f.ins().ref_test(ty);
            }
            (Rep::Uniform, Rep::Str) => {
                f.ins().ref_test(str);
            }
            (Rep::Uniform, Rep::StrOrNone) => {
                f.ins().ref_test_nullable(str);
            }
            // An object may be null.
            (Rep::Uniform, Rep::Object(class) | Rep::ObjectOrNone(class)) => {
                self.test_class(f, class, true);
            }
            // Each object of `from` is an object of `to`.
            (Rep::ObjectOrNone(from), Rep::Object(to)) if self.layout.descends(from, to) => {
                f.ins().ref_is_null().i32_eqz();
            }
            (Rep::Object(_) | Rep::ObjectOrNone(_), Rep::Object(class)) => {
                self.test_class(f, class, false);
            }
            _ => unreachable!("a test of {from:?} for {to:?}"),
        }
    }

    /// Pops a value of `from`, a union, and pushes the `bool` of whether it
    /// is a value of `to`, as `isinstance` finds. A null is no object, and
    /// a `bool` is an `int`.
    pub fn is_instance(&mut self, m: &mut ModuleState, f: &mut FuncState, from: Rep, to: Rep) {
        match (from, to) {
            (Rep::Uniform, Rep::Int) => {
                let bool = self.uniform.box_type(m, Rep::Bool);
                let value = f.temp(self.uniform.val_type());
                f.ins().local_tee(value);
                self.has(m, f, from, to);
                f.ins().local_get(value).ref_test(bool).i32_or();
                f.free(value);
            }
            (Rep::Uniform, Rep::Object(class)) => self.test_class(f, class, false),
            _ => self.has(m, f, from, to),
        }
    }

    /// The classes that are `class` or descend from it.
    fn descendants(&self, class: ClassId) -> Vec<ClassId> {
        (0..self.layout.classes())
            .map(ClassId::new)
            .filter(|other| self.layout.descends(*other, class))
            .collect()
    }

    /// Pops a reference and pushes the `bool` of whether it is an object
    /// of `class` or of a class that descends from it, or null when
    /// `nullable` is `true`. Each test names an exact type.
    fn test_class(&self, f: &mut FuncState, class: ClassId, nullable: bool) {
        let classes = self.descendants(class);
        if let [only] = *classes {
            let exact = self.layout.exact(only).heap_type();
            if nullable {
                f.ins().ref_test_nullable(exact);
            } else {
                f.ins().ref_test(exact);
            }
            return;
        }
        let value = f.temp(self.uniform.val_type());
        f.ins().local_tee(value);
        if nullable {
            f.ins().ref_is_null();
        } else {
            f.ins().drop().i32_const(0);
        }
        for class in classes {
            f.ins()
                .local_get(value)
                .ref_test(self.layout.exact(class).heap_type())
                .i32_or();
        }
        f.free(value);
    }

    /// Pops a reference that is null or an object of `class` or of a class
    /// that descends from it, and pushes it as a reference to the struct of
    /// `class`. Each cast names an exact type.
    fn cast_class(&self, f: &mut FuncState, class: ClassId) {
        if !self.layout.is_open(class) {
            f.ins().ref_cast(self.layout.exact(class).heap_type(), true);
            return;
        }
        let value = f.temp(self.uniform.val_type());
        f.ins().local_set(value);
        let done = f.block(BlockType::Result(self.class(class).ref_to(true)));
        f.ins().local_get(value);
        f.free(value);
        for class in self.descendants(class) {
            f.br_on_cast(done, self.layout.exact(class));
        }
        f.ins().unreachable();
        f.end();
    }

    /// Pops a value of `rep` and pushes the `bool` of whether it is `None`.
    pub fn is_none(&self, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::None => {
                f.ins().i32_const(1);
            }
            Rep::StrOrNone
            | Rep::Uniform
            | Rep::Object(_)
            | Rep::ObjectOrNone(_)
            | Rep::ListOrNone(_) => {
                f.ins().ref_is_null();
            }
            Rep::Int
            | Rep::Float
            | Rep::Bool
            | Rep::Str
            | Rep::List(_)
            | Rep::Hash
            | Rep::Func(_) => {
                f.ins().drop().i32_const(0);
            }
        }
    }

    /// Raises the exception of `message`.
    pub fn raise(&mut self, m: &mut ModuleState, f: &mut FuncState, message: &Message) {
        self.exc.raise(m, &mut self.str, f, message);
    }

    /// Pops an error of WASI and raises its `OSError`.
    pub fn raise_os_error(&self, f: &mut FuncState) {
        self.exc.raise_os_error(f);
    }

    /// Pops a `str`, which may be null, and raises an exception of `class`
    /// with it as the text of the argument.
    pub fn raise_text(&mut self, m: &mut ModuleState, f: &mut FuncState, class: Class) {
        self.exc.raise_text(m, &mut self.str, f, class);
    }

    /// Pushes the `str` with the text `s`.
    pub fn str_literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        self.str.literal(m, f, s);
    }

    pub fn int_literal(&self, f: &mut FuncState, value: i32) {
        self.int.literal(f, value);
    }

    pub fn float_literal(&self, f: &mut FuncState, value: f64) {
        self.float.literal(f, value);
    }

    pub fn bool_literal(&self, f: &mut FuncState, value: bool) {
        f.ins().i32_const(i32::from(value));
    }

    /// Pops a value of `rep` and pushes its truth as a `bool`. `None` is
    /// false, and an object of a union is true.
    pub fn truth(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.truth(f),
            Rep::Bool => {}
            Rep::Str => self.str.truth(f),
            Rep::Float => self.float.truth(f),
            Rep::None => {
                f.ins().i32_const(0);
            }
            Rep::StrOrNone => {
                let value = f.temp(self.str.val_type(true));
                f.ins().local_tee(value).ref_is_null();
                f.if_(BlockType::Result(ValType::I32));
                f.ins().i32_const(0);
                f.else_();
                f.ins().local_get(value);
                self.str.truth(f);
                f.end();
                f.free(value);
            }
            Rep::Uniform => {
                let func = match self.truth_uniform {
                    Some(func) => func,
                    None => {
                        let func = self.add_truth_uniform(m);
                        self.truth_uniform = Some(func);
                        func
                    }
                };
                f.ins().call(func);
            }
            // A function is true.
            Rep::Object(_) | Rep::ObjectOrNone(_) | Rep::Func(_) => {
                f.ins().ref_is_null().i32_eqz();
            }
            Rep::List(list) => {
                self.list.len(f, list.index());
                f.ins().i32_const(0).i32_ne();
            }
            Rep::ListOrNone(list) => self.list.truth(f, list.index()),
            Rep::Hash => unreachable!("the truth of a hash"),
        }
    }

    /// Defines the function that gives the truth of a value of a union,
    /// after the test of its type.
    fn add_truth_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        let ty = m.func_type(&[self.uniform.val_type()], &[ValType::I32]);
        let mut f = FuncState::new(1);
        let value = f.param(0);
        f.ins().local_get(value).ref_is_null();
        f.if_(BlockType::Empty);
        f.ins().i32_const(0).return_();
        f.end();
        let str = (Rep::Str, self.str.type_index().heap_type());
        for (rep, ty) in self.uniform.boxes(m).into_iter().chain([str]) {
            f.ins().local_get(value).ref_test(ty);
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, rep);
            self.truth(m, &mut f, rep);
            f.ins().return_();
            f.end();
        }
        f.ins().i32_const(1);
        m.add_func(ty, f.finish())
    }

    /// Pops an `int`, a `float` or a `bool`, as `rep` says, and pushes its
    /// `str`.
    pub fn str_of(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.str(f, &self.str),
            Rep::Float => self.float.repr(m, f, &mut self.str),
            Rep::Bool => {
                f.if_(BlockType::Result(self.str.val_type(false)));
                self.str.literal(m, f, "True");
                f.else_();
                self.str.literal(m, f, "False");
                f.end();
            }
            Rep::Str
            | Rep::None
            | Rep::StrOrNone
            | Rep::Uniform
            | Rep::Object(_)
            | Rep::ObjectOrNone(_)
            | Rep::List(_)
            | Rep::ListOrNone(_)
            | Rep::Hash
            | Rep::Func(_) => unreachable!("the str of {rep:?}"),
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
    pub fn int_arith(&mut self, m: &mut ModuleState, f: &mut FuncState, op: ArithOp) {
        self.int.arith(m, f, &mut self.str, &self.exc, op);
    }

    /// Pops an `i64` and pushes it as an `int`. A value outside the range
    /// of an `int` stops the program.
    pub fn i64_as_int(&self, f: &mut FuncState) {
        self.int.i64_as_int(f);
    }

    /// Pops two `int`s, or two `bool`s, and pushes the result of `op`.
    pub fn int_bits(&self, f: &mut FuncState, op: BitOp) {
        self.int.bits(f, op);
    }

    /// Pops an `int` and pushes its absolute value.
    pub fn int_abs(&self, f: &mut FuncState) {
        self.int.abs(f);
    }

    /// Pushes the `gcd` of no `int`, which [`Reprs::gcd_step`] takes with
    /// one `int` more, and [`Reprs::gcd_end`] gives as an `int`.
    pub fn gcd_start(&self, f: &mut FuncState) {
        self.int.gcd_start(f);
    }

    /// Pops a `gcd` and an `int`, and pushes the `gcd` of both.
    pub fn gcd_step(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.gcd_step(m, f);
    }

    /// Pops a `gcd` and pushes it as an `int`, which stops the program when
    /// it does not fit.
    pub fn gcd_end(&self, f: &mut FuncState) {
        self.int.gcd_end(f);
    }

    /// Pushes the `lcm` of no `int`, before the steps of `math.lcm`.
    pub fn lcm_start(&self, f: &mut FuncState) {
        self.int.lcm_start(f);
    }

    /// Pops an `lcm` and an `int` and pushes the `lcm` of both.
    pub fn lcm_step(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.lcm_step(m, f);
    }

    /// Pops an `lcm` and pushes it as an `int`, which stops the program when
    /// it does not fit.
    pub fn lcm_end(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.lcm_end(m, f, &mut self.str, &self.exc);
    }

    /// Pops an `int` and an exponent that is not negative, and pushes the
    /// power.
    pub fn int_pow(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.pow(m, f, &mut self.str, &self.exc);
    }

    /// Pops `a`, `b` and `m`, three `int`s with an `m` that is not 0, and
    /// pushes `a * b % m`.
    pub fn int_mul_mod(&self, f: &mut FuncState) {
        self.int.mul_mod(f);
    }

    /// Pops `n` and `k` and pushes `math.comb(n, k)` or `math.perm(n, k)`,
    /// by `choice`, or raises the error of a negative `int` or of a result
    /// that does not fit.
    pub fn choose(&mut self, m: &mut ModuleState, f: &mut FuncState, choice: Choice) {
        self.int.choose(m, f, &mut self.str, &self.exc, choice);
    }

    /// Pops an `int` and a number of digits, and pushes the `int` rounded
    /// to those digits.
    pub fn int_round(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.round(m, f);
    }

    /// Pops an `int` and pushes it as a `float`.
    pub fn int_as_float(&self, f: &mut FuncState) {
        self.int.as_i32(f);
        self.float.convert_i32(f);
    }

    /// Pops a `float` and pushes it as an `int`, without the fraction, or
    /// raises the error of a value that is not a number, is infinite or
    /// does not fit.
    pub fn float_as_int(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.convert_f64(m, f, &mut self.str, &self.exc);
    }

    /// Pops two `float`s and pushes the result of `op`.
    pub fn float_arith(&self, f: &mut FuncState, op: FloatOp) {
        self.float.arith(f, op);
    }

    /// Pops two `float`s and pushes the result of `op`, or raises
    /// `ZeroDivisionError` when the divisor is 0.
    pub fn float_div(&mut self, m: &mut ModuleState, f: &mut FuncState, op: DivOp) {
        self.float.div(m, f, &mut self.str, &self.exc, op);
    }

    /// Pops two `float`s and pushes the first to the power of the second,
    /// or raises the error of CPython.
    pub fn float_pow(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.pow(m, f, &mut self.str, &self.exc);
    }

    /// Pops a `float` and pushes its negation.
    pub fn float_neg(&self, f: &mut FuncState) {
        self.float.neg(f);
    }

    pub fn float_abs(&self, f: &mut FuncState) {
        self.float.abs(f);
    }

    /// Pops `x` and `y` and pushes `func` of them.
    pub fn float2_call(&self, m: &mut ModuleState, f: &mut FuncState, func: FloatFunc2) {
        self.float.float2_call(m, f, func);
    }

    pub fn hypot(&self, m: &mut ModuleState, f: &mut FuncState, count: usize) {
        self.float.hypot(m, f, count);
    }

    pub fn float_round(&self, f: &mut FuncState) {
        self.float.round(f);
    }

    /// Pops a `float` and pushes `func` of it, or raises the error of
    /// CPython.
    pub fn math_call(&mut self, m: &mut ModuleState, f: &mut FuncState, func: MathFunc) {
        self.float.math_call(m, f, &mut self.str, &self.exc, func);
    }

    /// Pops `x` and `y` and pushes `func` of them, or raises the error of
    /// CPython.
    pub fn math2_call(&mut self, m: &mut ModuleState, f: &mut FuncState, func: MathFunc2) {
        self.float.math2_call(m, f, &mut self.str, &self.exc, func);
    }

    /// Pops `x`, `y` and `z` and pushes `math.fma(x, y, z)`, or raises the
    /// error of CPython.
    pub fn fma(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.fma(m, f, &mut self.str, &self.exc);
    }

    /// Pops a `float` and an `int` and pushes `math.ldexp` of them, or
    /// raises `OverflowError` for a result too large for a `float`.
    pub fn ldexp(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.float.ldexp(m, f, &mut self.str, &self.exc);
    }

    /// Pops `x` and a base and pushes `math.log(x, base)`, or raises the
    /// error of CPython.
    pub fn log_base(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.log_base(m, f, &mut self.str, &self.exc);
    }

    /// Pops `x` and `y` and pushes `math.fmod(x, y)`, or raises the error
    /// of CPython.
    pub fn fmod(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.fmod(m, f, &mut self.str, &self.exc);
    }

    /// Pops `a`, `b`, `rel_tol` and `abs_tol` and pushes the `bool` of
    /// `math.isclose`, or raises `ValueError` for a negative tolerance.
    pub fn isclose(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.isclose(m, f, &mut self.str, &self.exc);
    }

    /// Pops a `float` and pushes its square root, or raises `ValueError`
    /// when it is below 0. `-0.0` and `nan` are not below 0.
    pub fn float_sqrt(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let x = f.temp(self.float.val_type());
        f.ins().local_tee(x);
        self.float.literal(f, 0.0);
        self.float.compare(f, CmpOp::Lt);
        f.if_(BlockType::Empty);
        self.raise(m, f, &MATH_DOMAIN_ERROR);
        f.end();
        f.ins().local_get(x);
        f.free(x);
        self.float.sqrt(f);
    }

    pub fn float_floor(&self, f: &mut FuncState) {
        self.float.floor(f);
    }

    pub fn float_ceil(&self, f: &mut FuncState) {
        self.float.ceil(f);
    }

    /// Pops a `float` and an `int`, and pushes the `float` rounded to that
    /// many digits, or raises `OverflowError`.
    pub fn float_round_digits(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.float.round_digits(m, f, &mut self.str, &self.exc);
    }

    /// Pops a `str` or a list, as `rep` says, and pushes its number of
    /// code points or of items as an `int`.
    pub fn len(&self, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Str => {
                self.str.len(f);
                self.int.len_as_int(f);
            }
            // The length of a list is not above its capacity, which an
            // `int` gave.
            Rep::List(list) => {
                self.list.len(f, list.index());
                self.int.small_as_int(f);
            }
            _ => unreachable!("the len of {rep:?}"),
        }
    }

    /// Pops a list and pushes the `int` of how many items its array has
    /// room for.
    pub fn list_capacity(&self, f: &mut FuncState, rep: Rep) {
        self.list.capacity(f, list_index(rep));
        self.int.small_as_int(f);
    }

    /// Pops a list and an `int` capacity, and gives the list an array with
    /// room for that many items, with its items.
    pub fn list_reserve(&self, f: &mut FuncState, rep: Rep) {
        self.int.as_i32(f);
        self.list.reserve(f, list_index(rep));
    }

    /// Pops a list and an `int`, and makes the list a list of that many
    /// items.
    pub fn list_set_len(&self, f: &mut FuncState, rep: Rep) {
        self.int.as_i32(f);
        self.list.set_len(f, list_index(rep));
    }

    /// Pops a list of `rep` and an `int`, not above its length, and makes
    /// the list a list of that many items, without a reference to the items
    /// past them.
    pub fn list_truncate(&self, f: &mut FuncState, rep: Rep) {
        self.int.as_i32(f);
        let item = self.list_items.get(list_index(rep));
        let null = match self.local_type(*item.expect("a type of list of the program")) {
            Some(ValType::Ref(ty)) => Some(ty.heap_type),
            _ => None,
        };
        self.list.truncate(f, list_index(rep), null);
    }

    /// Pops a list and pushes the array of its items.
    pub fn list_items(&self, f: &mut FuncState, rep: Rep) {
        self.list.items(f, list_index(rep));
    }

    /// Pops an `int` index and pushes it as the `i32` that an array takes.
    pub fn index_of(&self, f: &mut FuncState) {
        self.int.as_i32(f);
    }

    /// Pops a list and an `int` index, and pushes the item at the index, of
    /// `rep`. A negative index counts from the end, and an index out of
    /// range raises `IndexError`.
    pub fn list_index(&mut self, m: &mut ModuleState, f: &mut FuncState, list: Rep, rep: Rep) {
        self.checked_index(m, f, list, &LIST_INDEX_OUT_OF_RANGE);
        self.list_item(f, list, rep);
    }

    /// Pops a list and an `int` index, and sets the item at the index to
    /// the value in `item`. A negative index counts from the end, and an
    /// index out of range raises `IndexError`.
    pub fn list_set(&mut self, m: &mut ModuleState, f: &mut FuncState, list: Rep, item: LocalIdx) {
        self.checked_index(m, f, list, &LIST_ASSIGNMENT_OUT_OF_RANGE);
        f.ins().local_get(item);
        self.set_list_item(f, list);
    }

    /// Pops a list and an `int` index, and pushes the array of the items
    /// and the index in range as an `i32`. A negative index counts from the
    /// end, and an index out of range raises `IndexError` with `message`.
    fn checked_index(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        list: Rep,
        message: &Message,
    ) {
        let index = list_index(list);
        self.int.as_i32(f);
        let i = f.temp(ValType::I32);
        let value = f.temp(self.list.list_type(index).ref_to(true));
        f.ins().local_set(i).local_set(value);
        f.ins().local_get(i).i32_const(0).i32_lt_s();
        f.if_(BlockType::Empty);
        f.ins().local_get(i).local_get(value);
        self.list.len(f, index);
        f.ins().i32_add().local_set(i);
        f.end();
        // A negative index past the start is a large unsigned one.
        f.ins().local_get(i).local_get(value);
        self.list.len(f, index);
        f.ins().i32_ge_u();
        f.if_(BlockType::Empty);
        self.raise(m, f, message);
        f.end();
        f.ins().local_get(value);
        self.list.items(f, index);
        f.ins().local_get(i);
        f.free(i);
        f.free(value);
    }

    /// Pops the array of the items of a list and an index of
    /// [`Self::index_of`], and pushes the item at the index, of `rep`.
    pub fn list_item(&self, f: &mut FuncState, list: Rep, rep: Rep) {
        self.list.item(f, list_index(list));
        // An array holds an item in the form of a local.
        non_null(f, rep);
    }

    /// Pops the array of the items of a list, an index of [`Self::index_of`]
    /// and an item, and sets the item of the array at the index.
    pub fn set_list_item(&self, f: &mut FuncState, rep: Rep) {
        self.list.set_item(f, list_index(rep));
    }

    /// Pops the array of a list of `rep`, an `i32` index, the array of
    /// another list of `rep`, an `i32` index and an `i32` count, and copies
    /// that many items of the second array to the first one.
    pub fn copy_list_items(&self, f: &mut FuncState, rep: Rep) {
        self.list.copy_items(f, list_index(rep));
    }

    /// Starts a literal of the type `list` with `count` items, as
    /// [`ListRepr::begin_literal`] says.
    pub fn begin_literal(&self, f: &mut FuncState, list: ListId, count: usize) -> Option<LocalIdx> {
        self.list.begin_literal(f, list.index(), count)
    }

    /// Comes before the item at `index` of a literal.
    pub fn before_item(&self, f: &mut FuncState, array: Option<LocalIdx>, index: usize) {
        self.list.before_item(f, array, index);
    }

    /// Comes after an item of a literal of the type `list`.
    pub fn after_item(&self, f: &mut FuncState, list: ListId, array: Option<LocalIdx>) {
        self.list.after_item(f, list.index(), array);
    }

    /// Pushes the list of a literal of the type `list` with `count` items.
    pub fn end_literal(
        &self,
        f: &mut FuncState,
        list: ListId,
        count: usize,
        array: Option<LocalIdx>,
    ) {
        self.list.end_literal(f, list.index(), count, array);
    }

    /// Pops a `str` and an `int`, and pushes the `str` repeated that many
    /// times. A result too long for memory raises `MemoryError`.
    pub fn str_repeat(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.repeat(f);
        self.raise_if_null(m, f, &MEMORY_ERROR);
    }

    /// Marks that the tests run, so an exception from here on is not one of
    /// the body of the file.
    pub fn tests_run(&self, f: &mut FuncState) {
        self.exc.tests_run(f);
    }

    /// Opens the body of an example of a docstring, whose output goes to
    /// the buffer of `capture`. [`Self::end_example`] closes it with the
    /// label that this returns.
    pub fn begin_example(&self, f: &mut FuncState, capture: &Capture) -> Label {
        // An example before this one may have raised inside a `repr`, which
        // leaves objects on the stack.
        if let Some((node, top)) = self.repr_stack {
            f.ins().ref_null(node.heap_type()).global_set(top);
        }
        self.exc.begin_example(f, capture)
    }

    /// Closes the body of an example, and keeps what it wrote and the
    /// exception that ended it.
    pub fn end_example(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        capture: &Capture,
        done: Label,
    ) {
        self.exc.end_example(m, &mut self.str, f, capture, done);
    }

    /// Pushes what the last example wrote, as a `str`.
    pub fn captured(&self, f: &mut FuncState) {
        self.exc.captured(f);
    }

    /// Pushes the last line of the traceback of the exception of the last
    /// example, with its `\n`, or an empty `str`.
    pub fn captured_exception(&self, f: &mut FuncState) {
        self.exc.captured_exception(f);
    }

    /// Pops a `str` and writes it, as one call of `write` in CPython.
    pub fn str_write(&self, f: &mut FuncState) {
        self.str.write(f);
    }

    /// Pops an `int`, and ends the program with the exit code 0 when the
    /// `int` is 0, and with 1 otherwise.
    pub fn exit(&self, f: &mut FuncState) {
        self.int.as_i32(f);
        self.exc.exit(f);
    }

    /// Pops an `int` and pushes a new `str` of that many code points, each
    /// of them 0.
    pub fn str_new(&self, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.new_zeroed(f);
    }

    /// Pops a `str`, an `int` index in range and an `int` code point, and
    /// sets the code point of the `str` at the index.
    pub fn str_set(&self, f: &mut FuncState) {
        // An `int` is the `i32` that the `str` holds for a code point, and
        // that an index is.
        self.str.set(f);
    }

    /// Pops a `str` and an `int` index in range, and pushes the code point
    /// at the index as an `int`.
    pub fn code_point(&self, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.get(f);
        self.int.small_as_int(f);
    }

    /// Pops a `str` of ASCII in the form of a `float` of Python without
    /// underscores, and pushes its `float`.
    pub fn parse_float(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.float.parse(m, f, &self.str);
    }

    /// Pops an `int` mask that is not negative and a `hash`, and pushes the
    /// bits of the `hash` that the mask keeps, as an `int`.
    pub fn hash_slot(&self, f: &mut FuncState) {
        self.int.as_i32(f);
        f.ins().i64_extend_i32_u().i64_and().i32_wrap_i64();
        self.int.small_as_int(f);
    }

    /// Pops a value of `rep` and pushes its `hash`, an `i64`, as CPython
    /// gives it. `None` has no value to pop.
    pub fn hash(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep) {
        match rep {
            // An `int` is its own hash, except -1, which CPython keeps for
            // an error. `x - (x == -1)` takes -1 to -2.
            Rep::Int => {
                self.int.as_i32(f);
                let x = f.temp(ValType::I32);
                f.ins()
                    .local_tee(x)
                    .local_get(x)
                    .i32_const(-1)
                    .i32_eq()
                    .i32_sub()
                    .i64_extend_i32_s();
                f.free(x);
            }
            Rep::Bool => {
                f.ins().i64_extend_i32_u();
            }
            Rep::None => {
                f.ins().i64_const(NONE_HASH);
            }
            Rep::Float => {
                let builtin = m.builtin("f64_hash");
                f.ins().call(builtin);
            }
            Rep::Str => self.str.hash(m, f),
            Rep::StrOrNone => {
                let value = f.temp(self.str.val_type(true));
                f.ins().local_tee(value).ref_is_null();
                f.if_(BlockType::Result(ValType::I64));
                f.ins().i64_const(NONE_HASH);
                f.else_();
                f.ins().local_get(value).ref_as_non_null();
                self.str.hash(m, f);
                f.end();
                f.free(value);
            }
            Rep::Uniform => {
                let func = match self.hash_uniform {
                    Some(func) => func,
                    None => {
                        let func = self.add_hash_uniform(m);
                        self.hash_uniform = Some(func);
                        func
                    }
                };
                f.ins().call(func);
            }
            _ => unreachable!("the hash of {rep:?}"),
        }
    }

    /// Defines the function that gives the `hash` of a value of a union
    /// that is `None`, a number or a `str`. The lowering tests the classes
    /// of an object before it.
    fn add_hash_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        let ty = m.func_type(&[self.uniform.val_type()], &[ValType::I64]);
        let mut f = FuncState::new(1);
        let value = f.param(0);
        f.ins().local_get(value).ref_is_null();
        f.if_(BlockType::Empty);
        f.ins().i64_const(NONE_HASH).return_();
        f.end();
        let str = (Rep::Str, self.str.type_index().heap_type());
        for (rep, ty) in self.uniform.boxes(m).into_iter().chain([str]) {
            f.ins().local_get(value).ref_test(ty);
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, rep);
            self.hash(m, &mut f, rep);
            f.ins().return_();
            f.end();
        }
        f.ins().unreachable();
        m.add_func(ty, f.finish())
    }

    /// Pushes the start of the `hash` of a tuple, as `tuplehash` of
    /// `Objects/tupleobject.c` of CPython gives it, an xxHash of the
    /// `hash` of each item.
    pub fn hash_tuple_start(&self, f: &mut FuncState) {
        f.ins().i64_const(XXPRIME_5 as i64);
    }

    /// Pops the `hash` of a tuple up to an item and the `hash` of the
    /// item, and pushes the `hash` of the tuple up to the item.
    pub fn hash_tuple_item(&self, f: &mut FuncState) {
        f.ins()
            .i64_const(XXPRIME_2 as i64)
            .i64_mul()
            .i64_add()
            .i64_const(31)
            .i64_rotl()
            .i64_const(XXPRIME_1 as i64)
            .i64_mul();
    }

    /// Pops the `hash` of a tuple of `len` items up to its last item, and
    /// pushes the `hash` of the tuple. CPython keeps -1 for an error, and
    /// takes it to 1546275796.
    pub fn hash_tuple_end(&self, f: &mut FuncState, len: usize) {
        let len = u64::try_from(len).expect("a length in 64 bits");
        let h = f.temp(ValType::I64);
        f.ins()
            .i64_const((len ^ XXPRIME_5 ^ 3527539) as i64)
            .i64_add()
            .local_set(h)
            .i64_const(1546275796)
            .local_get(h)
            .local_get(h)
            .i64_const(-1)
            .i64_eq()
            .select();
        f.free(h);
    }

    /// Pops an `int`, a code point, and pushes the `int` of its value as a
    /// decimal digit, or -1.
    pub fn decimal_value(&self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        let builtin = m.builtin("str_decimal_value");
        f.ins().call(builtin);
        self.int.small_as_int(f);
    }

    /// Pops an `int`, a code point, and pushes the `bool` of whether it is
    /// in `set`.
    pub fn code_point_is(&self, m: &mut ModuleState, f: &mut FuncState, set: CodePointSet) {
        self.int.as_i32(f);
        let builtin = m.builtin(set.builtin());
        f.ins().call(builtin);
    }

    /// Pops two `int`, a code point and an index, and pushes the `int` of
    /// the code point at the index of the mapping `case` of the code point,
    /// or -1 past its end.
    pub fn case_at(&self, m: &mut ModuleState, f: &mut FuncState, case: Case) {
        self.int.as_i32(f);
        let index = f.temp(ValType::I32);
        f.ins().local_set(index);
        self.int.as_i32(f);
        f.ins().local_get(index);
        f.free(index);
        let builtin = m.builtin(case.builtin());
        f.ins().call(builtin);
        self.int.small_as_int(f);
    }

    /// Pops an `int`, the `stop` of a slice, and pushes it as
    /// [`Self::str_slice`] takes it.
    pub fn slice_stop(&self, f: &mut FuncState) {
        self.int.as_i32(f);
    }

    /// Pushes the `stop` of a slice that goes to the end of the `str`, as
    /// [`Self::str_slice`] takes it. No `str` is longer than this `stop`.
    pub fn slice_to_the_end(&self, f: &mut FuncState) {
        f.ins().i32_const(i32::MAX);
    }

    /// Pops a `str`, an `int` `start` and a `stop`, and pushes the slice of
    /// the `str` between the two.
    pub fn str_slice(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        let stop = f.temp(ValType::I32);
        f.ins().local_set(stop);
        self.int.as_i32(f);
        f.ins().local_get(stop);
        f.free(stop);
        self.str.slice(m, f);
    }

    /// Pops a `str` and an `int`, and pushes the code point at that index
    /// as a `str`. An index out of range raises `IndexError`.
    pub fn str_index(&mut self, m: &mut ModuleState, f: &mut FuncState) {
        self.int.as_i32(f);
        self.str.index(f);
        self.raise_if_null(m, f, &STR_INDEX_OUT_OF_RANGE);
    }

    /// Raises an exception with `message` when the `str` on the stack is
    /// null, and leaves it as a `str` otherwise.
    fn raise_if_null(&mut self, m: &mut ModuleState, f: &mut FuncState, message: &Message) {
        let result = f.temp(self.str.val_type(true));
        f.ins().local_tee(result).ref_is_null();
        f.if_(BlockType::Empty);
        self.raise(m, f, message);
        f.end();
        self.load(f, Rep::Str, result);
        f.free(result);
    }

    /// Pops two values of `rep` and pushes the `bool` of `op`.
    pub fn compare(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep, op: CmpOp) {
        match rep {
            Rep::Uniform => {
                let compare = self.compare_uniform(m);
                f.ins().call(compare);
                // -1, 0 and 1 are the order, and 2 is no order.
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.i32_eqz(),
                    CmpOp::Ne => ins.i32_const(0).i32_ne(),
                    CmpOp::Lt => ins.i32_const(-1).i32_eq(),
                    CmpOp::Gt => ins.i32_const(1).i32_eq(),
                    CmpOp::Le => ins.i32_const(1).i32_add().i32_const(1).i32_le_u(),
                    CmpOp::Ge => ins.i32_const(1).i32_le_u(),
                };
            }
            Rep::Int => self.int.compare(f, op),
            // `a op b` is `compare(a, b) op 0`.
            Rep::Str => {
                self.str.compare(f);
                self.int.literal(f, 0);
                self.int.compare(f, op);
            }
            Rep::Float => self.float.compare(f, op),
            // `False` and `True` are the `i32` 0 and 1.
            Rep::Bool => {
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.i32_eq(),
                    CmpOp::Ne => ins.i32_ne(),
                    CmpOp::Lt => ins.i32_lt_u(),
                    CmpOp::Le => ins.i32_le_u(),
                    CmpOp::Gt => ins.i32_gt_u(),
                    CmpOp::Ge => ins.i32_ge_u(),
                };
            }
            // A member of an `Enum` and a function are equal to themselves
            // alone.
            Rep::Object(_) | Rep::ObjectOrNone(_) | Rep::Func(_) => {
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.ref_eq(),
                    CmpOp::Ne => ins.ref_eq().i32_eqz(),
                    _ => unreachable!("an order of objects"),
                };
            }
            Rep::None | Rep::StrOrNone | Rep::List(_) | Rep::ListOrNone(_) | Rep::Hash => {
                unreachable!("a comparison of {rep:?}")
            }
        }
    }

    /// Pops a value of `rep` and writes what `str` gives for it to the
    /// standard output. `None` has no value to pop.
    pub fn write(&mut self, m: &mut ModuleState, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.write(f, &self.str),
            Rep::Float => self.float.write(m, f),
            Rep::Bool => {
                f.if_(BlockType::Empty);
                self.str.literal(m, f, "True");
                self.str.write(f);
                f.else_();
                self.str.literal(m, f, "False");
                self.str.write(f);
                f.end();
            }
            Rep::Str => self.str.write(f),
            Rep::StrOrNone => {
                let value = f.temp(self.str.val_type(true));
                f.ins().local_tee(value).ref_is_null();
                f.if_(BlockType::Empty);
                self.write(m, f, Rep::None);
                f.else_();
                f.ins().local_get(value).ref_as_non_null();
                self.str.write(f);
                f.end();
                f.free(value);
            }
            Rep::Uniform => {
                let write = match self.write_uniform {
                    Some(write) => write,
                    None => {
                        let write = self.add_write_uniform(m);
                        self.write_uniform = Some(write);
                        write
                    }
                };
                f.ins().call(write);
            }
            Rep::None => {
                self.str.literal(m, f, "None");
                self.str.write(f);
            }
            // An object may be null.
            Rep::Object(class) | Rep::ObjectOrNone(class) => {
                let value = f.temp(self.class(class).ref_to(true));
                f.ins().local_tee(value).ref_is_null();
                f.if_(BlockType::Empty);
                self.write(m, f, Rep::None);
                f.else_();
                f.ins().local_get(value).call(self.class_str(class));
                self.str.write(f);
                f.end();
                f.free(value);
            }
            // The `str` of a list is its `repr`.
            Rep::List(list) => {
                f.ins().call(self.list_repr(list));
                self.str.write(f);
            }
            Rep::ListOrNone(list) => {
                let repr = self.list_repr(list);
                let value = f.temp(self.list_type(list).ref_to(true));
                f.ins().local_tee(value).ref_is_null();
                f.if_(BlockType::Empty);
                self.write(m, f, Rep::None);
                f.else_();
                f.ins().local_get(value).ref_as_non_null().call(repr);
                self.str.write(f);
                f.end();
                f.free(value);
            }
            Rep::Hash => unreachable!("the prelude writes no hash"),
            Rep::Func(_) => unreachable!("a write of a function"),
        }
    }

    /// The function that gives the `repr` of a list of the type `list`.
    fn list_repr(&self, list: ListId) -> FuncIdx {
        self.list_reprs
            .get(list.index())
            .copied()
            .flatten()
            .expect("the repr of a list that the program writes")
    }

    /// The function that gives the `str` of an object of `class`.
    fn class_str(&self, class: ClassId) -> FuncIdx {
        self.class_strs
            .get(class.index())
            .copied()
            .flatten()
            .expect("the str of a class whose objects the program writes")
    }

    /// Defines the function that compares two values of a union. It gives
    /// -1, 0 or 1 as the first value comes before, with or after the
    /// second, and 2 when they have no order. An object is equal to itself
    /// alone, and has no order with any value. Two numbers compare as
    /// `float`s, which is exact for an `int` of 31 bits, and a NaN has no
    /// order. Two `str`s compare by their code points. `None` is equal to
    /// `None` alone, and a number and a `str` have no order. The checker
    /// rejects an order between values without one, so only `==` and `!=`
    /// see a 2 that is not a NaN.
    fn compare_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        if let Some(compare) = self.compare_uniform {
            return compare;
        }
        let compare = self.add_compare_uniform(m);
        self.compare_uniform = Some(compare);
        compare
    }

    /// Pops two uniform values and pushes the `bool` of whether they are
    /// equal. Two objects of a class of `eqs` go to the function beside the
    /// class, which may raise, and the rest compare as `==` of a uniform
    /// value does.
    pub fn uniform_eq(
        &mut self,
        m: &mut ModuleState,
        f: &mut FuncState,
        eqs: Vec<(ClassId, FuncIdx)>,
    ) {
        let func = match self.uniform_eqs.get(&eqs) {
            Some(func) => *func,
            None => {
                let func = self.add_uniform_eq(m, &eqs);
                self.uniform_eqs.insert(eqs, func);
                func
            }
        };
        f.ins().call(func);
    }

    fn add_uniform_eq(&mut self, m: &mut ModuleState, eqs: &[(ClassId, FuncIdx)]) -> FuncIdx {
        let uniform = self.uniform.val_type();
        let ty = m.func_type(&[uniform, uniform], &[ValType::I32]);
        let mut f = FuncState::new(2);
        let (a, b) = (f.param(0), f.param(1));
        for &(class, eq) in eqs {
            let heap = self.layout.exact(class).heap_type();
            f.ins()
                .local_get(a)
                .ref_test(heap)
                .local_get(b)
                .ref_test(heap)
                .i32_and();
            f.if_(BlockType::Empty);
            for value in [a, b] {
                f.ins().local_get(value).ref_cast(heap, false);
            }
            f.ins().call(eq).return_();
            f.end();
        }
        let compare = self.compare_uniform(m);
        f.ins().local_get(a).local_get(b).call(compare).i32_eqz();
        m.add_func(ty, f.finish())
    }

    fn add_compare_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        let uniform = self.uniform.val_type();
        let ty = m.func_type(&[uniform, uniform], &[ValType::I32]);
        let mut f = FuncState::new(2);
        let (a, b) = (f.param(0), f.param(1));
        let unordered = 2;
        f.ins()
            .local_get(a)
            .ref_is_null()
            .local_get(b)
            .ref_is_null()
            .i32_or();
        f.if_(BlockType::Empty);
        f.ins()
            .local_get(a)
            .ref_is_null()
            .local_get(b)
            .ref_is_null()
            .i32_and()
            .i32_eqz()
            .i32_const(unordered)
            .i32_mul()
            .return_();
        f.end();
        let str = self.str.type_index().heap_type();
        f.ins().local_get(a).ref_test(str);
        f.ins().local_get(b).ref_test(str);
        f.ins().i32_add();
        let strs = f.temp(ValType::I32);
        f.ins().local_tee(strs).i32_const(1).i32_eq();
        f.if_(BlockType::Empty);
        f.ins().i32_const(unordered).return_();
        f.end();
        f.ins().local_get(strs).i32_const(2).i32_eq();
        f.if_(BlockType::Empty);
        f.ins().local_get(a);
        self.narrow(m, &mut f, Rep::Uniform, Rep::Str);
        f.ins().local_get(b);
        self.narrow(m, &mut f, Rep::Uniform, Rep::Str);
        self.str.compare(&mut f);
        f.ins().return_();
        f.end();
        f.free(strs);
        // Two objects of a copy, as two `dict`s of the same types, compare
        // with the `__eq__` of the copy.
        for &(class, eq) in &self.copy_eqs {
            let heap = self.layout.exact(class).heap_type();
            f.ins()
                .local_get(a)
                .ref_test(heap)
                .local_get(b)
                .ref_test(heap)
                .i32_and();
            f.if_(BlockType::Empty);
            f.ins()
                .local_get(a)
                .ref_cast(heap, false)
                .local_get(b)
                .call(eq)
                .i32_eqz()
                .i32_const(unordered)
                .i32_mul()
                .return_();
            f.end();
        }
        // An object is equal to itself alone.
        let boxes = self.uniform.boxes(m);
        for value in [a, b] {
            for (i, (_, box_type)) in boxes.iter().enumerate() {
                f.ins().local_get(value).ref_test(*box_type);
                if i > 0 {
                    f.ins().i32_or();
                }
            }
        }
        f.ins().i32_and().i32_eqz();
        f.if_(BlockType::Empty);
        f.ins()
            .local_get(a)
            .local_get(b)
            .ref_eq()
            .i32_eqz()
            .i32_const(unordered)
            .i32_mul()
            .return_();
        f.end();
        let (x, y) = (f.temp(ValType::F64), f.temp(ValType::F64));
        for (value, number) in [(a, x), (b, y)] {
            self.uniform_as_f64(m, &mut f, value);
            f.ins().local_set(number);
        }
        f.ins().local_get(x).local_get(y).f64_lt();
        f.if_(BlockType::Empty);
        f.ins().i32_const(-1).return_();
        f.end();
        f.ins().local_get(x).local_get(y).f64_gt();
        f.if_(BlockType::Empty);
        f.ins().i32_const(1).return_();
        f.end();
        f.ins()
            .local_get(x)
            .local_get(y)
            .f64_eq()
            .i32_eqz()
            .i32_const(unordered)
            .i32_mul();
        m.add_func(ty, f.finish())
    }

    /// Pushes the number in `value`, a boxed `int`, `float` or `bool`, as
    /// an `f64`.
    fn uniform_as_f64(&mut self, m: &mut ModuleState, f: &mut FuncState, value: LocalIdx) {
        let mut ends = 0;
        for (rep, box_type) in self.uniform.boxes(m) {
            if rep == Rep::Float {
                continue;
            }
            f.ins().local_get(value).ref_test(box_type);
            f.if_(BlockType::Result(ValType::F64));
            f.ins().local_get(value);
            self.narrow(m, f, Rep::Uniform, rep);
            if rep == Rep::Int {
                self.int.as_i32(f);
            }
            // `False` and `True` are the `i32` 0 and 1.
            f.ins().f64_convert_i32_s();
            f.else_();
            ends += 1;
        }
        f.ins().local_get(value);
        self.narrow(m, f, Rep::Uniform, Rep::Float);
        for _ in 0..ends {
            f.end();
        }
    }

    /// Defines the function that writes a value of a union, after the
    /// test of its type.
    fn add_write_uniform(&mut self, m: &mut ModuleState) -> FuncIdx {
        let classes = self.str_classes();
        self.add_uniform_dispatch(m, &[], classes, |reprs, m, f, rep| reprs.write(m, f, rep))
    }

    /// The classes with a function for the `str` of their objects. The
    /// `print` of a union makes it for each class in the type of the
    /// union.
    fn str_classes(&self) -> Vec<ClassId> {
        (0..self.layout.classes())
            .filter(|&i| self.class_strs.get(i).copied().flatten().is_some())
            .map(ClassId::new)
            .collect()
    }
}

/// The index of the type of a list of `rep`.
fn list_index(rep: Rep) -> usize {
    rep.list().expect("a list").index()
}

/// Pops a value of `rep` in the form of a local and pushes it in the form
/// of [`Reprs::value_type`], which is non-null for a `str` and a list.
fn non_null(f: &mut FuncState, rep: Rep) {
    if matches!(rep, Rep::Str | Rep::List(_)) {
        f.ins().ref_as_non_null();
    }
}
