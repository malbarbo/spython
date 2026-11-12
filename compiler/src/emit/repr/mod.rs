//! How each value is laid out in wasm. No other module knows it.

mod float;
mod int;
mod list;
pub(super) mod str;
mod uniform;

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, StorageType, ValType};

use super::module::{
    Composite, FuncIdx, FuncState, GlobalIdx, Ins, Label, LocalIdx, ModuleState, TypeIdx,
};
use super::runtime::exc::Exceptions;
use super::runtime::host::{Capture, Host};
use crate::cpython::{
    Class, LIST_ASSIGNMENT_OUT_OF_RANGE, LIST_INDEX_OUT_OF_RANGE, MATH_DOMAIN_ERROR, MEMORY_ERROR,
    Message, STR_INDEX_OUT_OF_RANGE,
};
use crate::ir::{
    ArithOp, Case, Choice, ClassId, CmpOp, CodePointSet, DivOp, FloatFunc2, FloatOp, ListId,
    ListType, Literal, MathFunc, MathFunc2, Rep, SiteFrame, UserClass,
};
use float::FloatRepr;
use int::IntRepr;
use list::ListRepr;
use str::StrRepr;
use uniform::UniformRepr;

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
    /// `(func (param (ref null eq)) (result i32))`, which tells whether
    /// the `repr` of an object runs.
    in_repr: Option<FuncIdx>,
    exc: Exceptions,
    /// The struct of an object of each class of the program, by its
    /// [`ClassId`].
    classes: Vec<TypeIdx>,
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
    /// The function that gives the `repr` of a list of each type, when the
    /// program writes one.
    list_reprs: Vec<Option<FuncIdx>>,
    /// The struct of a node of the stack of the objects whose `repr` runs,
    /// with the object and the node under it, and the global of the top of
    /// the stack, when a class guards its `repr`.
    repr_stack: Option<(TypeIdx, GlobalIdx)>,
}

impl Reprs {
    /// The representations of a module with the classes `classes` and the
    /// types of lists `lists`, which runs doctests when `tests` is `true`.
    pub fn new(
        m: &mut ModuleState,
        host: &Host,
        tests: bool,
        classes: &[UserClass],
        lists: &[ListType],
    ) -> Self {
        let mut str = StrRepr::ucs4(m, host);
        let exc = Exceptions::new(m, host, &mut str, tests);
        let int = IntRepr::i31(m, &mut str, &exc);
        let float = FloatRepr::f64(host);
        let mut reprs = Reprs {
            int,
            float,
            str,
            uniform: UniformRepr::new(),
            list: ListRepr::empty(),
            write_uniform: None,
            repr_uniform: None,
            compare_uniform: None,
            uniform_eqs: HashMap::new(),
            in_repr: None,
            exc,
            classes: Vec::new(),
            members: Vec::new(),
            class_strs: Vec::new(),
            class_reprs: Vec::new(),
            str_repr: None,
            list_reprs: Vec::new(),
            repr_stack: None,
        };
        // A field of a class may hold an object of any class, its own too,
        // or a list, and an item of a list may be an object or another list,
        // so the types refer to each other by the indices that they will have
        // in their rec group. The struct and the array of each list come
        // after the classes.
        let first = m.next_type_index();
        reprs.classes = (0..classes.len()).map(|i| first.plus(i)).collect();
        let list_types: Vec<(TypeIdx, TypeIdx)> = (0..lists.len())
            .map(|i| {
                let list = first.plus(classes.len() + 2 * i);
                (list, list.plus(1))
            })
            .collect();
        reprs.list = ListRepr::new(m, &list_types);
        let mut composites: Vec<Composite> = classes
            .iter()
            .map(|class| {
                Composite::Struct(
                    class
                        .fields
                        .iter()
                        .map(|rep| {
                            let ty = reprs.value_type(*rep).expect("a field has a value");
                            (StorageType::Val(ty), class.mutable)
                        })
                        .collect(),
                )
            })
            .collect();
        for (list, (ty, _)) in lists.iter().zip(&list_types) {
            let item = reprs.local_type(list.item).expect("an item has a value");
            composites.extend(ListRepr::composites(*ty, item));
        }
        let node = first.plus(classes.len() + 2 * lists.len());
        let guarded = classes.iter().any(|class| class.repr_guard);
        if guarded {
            let object = reprs.uniform.val_type();
            composites.push(Composite::Struct(vec![
                (StorageType::Val(object), false),
                (StorageType::Val(node.ref_to(true)), false),
            ]));
        }
        let mut group = m.rec_group(&composites);
        if guarded {
            let top = m.global(node.ref_to(true), &ConstExpr::ref_null(node.heap_type()));
            reprs.repr_stack = Some((group.pop().expect("the node of the stack"), top));
        }
        let listed: Vec<TypeIdx> = list_types.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let group_lists = group.split_off(classes.len());
        debug_assert_eq!(group_lists, listed, "the lists in their rec group");
        debug_assert_eq!(group, reprs.classes, "the classes in their rec group");
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
        for (field, text) in fields.iter().zip(texts) {
            match (field, text) {
                (Literal::Int(value), _) => self.int.init_literal(m, *value),
                (Literal::Str(_), text) => {
                    let text = text.expect("the global of the text");
                    Ins::new(&mut m.init).global_get(text).ref_as_non_null();
                }
            }
        }
        Ins::new(&mut m.init).struct_new(ty).global_set(global);
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
        let ty = m.func_type(&[self.uniform.val_type()], &[self.str.val_type(false)]);
        let mut f = FuncState::new(1);
        let value = f.param(0);
        f.ins().local_get(value).ref_is_null();
        f.if_(BlockType::Empty);
        self.str.literal(m, &mut f, "None");
        f.ins().return_();
        f.end();
        for (rep, box_type) in self.uniform.boxes(m) {
            f.ins().local_get(value).ref_test(box_type);
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, rep);
            self.str_of(m, &mut f, rep);
            f.ins().return_();
            f.end();
        }
        let str = self.str.type_index().heap_type();
        f.ins().local_get(value).ref_test(str);
        f.if_(BlockType::Empty);
        f.ins().local_get(value);
        self.narrow(m, &mut f, Rep::Uniform, Rep::Str);
        f.ins()
            .call(self.str_repr.expect("the repr of a str"))
            .return_();
        f.end();
        for (i, repr) in self.class_reprs.clone().into_iter().enumerate() {
            let class = ClassId::new(i);
            let repr = repr.expect("the repr of each class");
            f.ins()
                .local_get(value)
                .ref_test(self.class(class).heap_type());
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, Rep::Object(class));
            f.ins().call(repr).return_();
            f.end();
        }
        f.ins().unreachable();
        m.add_func(ty, f.finish())
    }

    /// Pops an object of `class` and pushes its field at `field`. A field
    /// has the type of a value of its rep.
    pub fn field(&self, f: &mut FuncState, class: ClassId, field: u32) {
        f.ins().struct_get(self.class(class), field);
    }

    /// Pops a value for each field of `class` and pushes a new object of
    /// `class` with them. A value of each rep is a value of its field.
    pub fn new_object(&self, f: &mut FuncState, class: ClassId) {
        f.ins().struct_new(self.class(class));
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
            Rep::None => None,
        }
    }

    /// The struct of a list of the type `list`.
    fn list_type(&self, list: ListId) -> TypeIdx {
        self.list.list_type(list.index())
    }

    /// The struct of an object of `class`.
    fn class(&self, class: ClassId) -> TypeIdx {
        *self
            .classes
            .get(class.index())
            .expect("a class of the program")
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
            (Rep::Object(a), Rep::ObjectOrNone(b)) if a == b => {}
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
            (Rep::Uniform, Rep::Object(class) | Rep::ObjectOrNone(class)) => {
                f.ins().ref_cast(self.class(class).heap_type(), true);
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
                f.ins().ref_test_nullable(self.class(class).heap_type());
            }
            _ => unreachable!("a test of {from:?} for {to:?}"),
        }
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
            Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::List(_) => {
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

    /// Pops a value of `rep` and pushes its truth as a `bool`.
    pub fn truth(&self, f: &mut FuncState, rep: Rep) {
        match rep {
            Rep::Int => self.int.truth(f),
            Rep::Bool => {}
            Rep::Str => self.str.truth(f),
            Rep::Float => self.float.truth(f),
            Rep::Object(_) | Rep::ObjectOrNone(_) => {
                f.ins().ref_is_null().i32_eqz();
            }
            Rep::List(list) | Rep::ListOrNone(list) => self.list.truth(f, list.index()),
            Rep::None | Rep::StrOrNone | Rep::Uniform => unreachable!("the truth of {rep:?}"),
        }
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
            | Rep::ListOrNone(_) => unreachable!("the str of {rep:?}"),
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

    /// Pops a list of items of `item` and an `int`, not above its length,
    /// and makes the list a list of that many items, without a reference
    /// to the items past them.
    pub fn list_truncate(&self, f: &mut FuncState, rep: Rep, item: Rep) {
        self.int.as_i32(f);
        let null = match self.local_type(item) {
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
            // A member of an `Enum` is equal to itself alone.
            Rep::Object(_) | Rep::ObjectOrNone(_) => {
                let mut ins = f.ins();
                match op {
                    CmpOp::Eq => ins.ref_eq(),
                    CmpOp::Ne => ins.ref_eq().i32_eqz(),
                    _ => unreachable!("an order of objects"),
                };
            }
            Rep::None | Rep::StrOrNone | Rep::List(_) | Rep::ListOrNone(_) => {
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
            // The `str` of a list is its `repr`, and a list or `None` may be
            // null.
            Rep::List(list) | Rep::ListOrNone(list) => {
                let repr = self
                    .list_reprs
                    .get(list.index())
                    .copied()
                    .flatten()
                    .expect("the repr of a list that the program writes");
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
        }
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
            let heap = self.class(class).heap_type();
            f.ins()
                .local_get(a)
                .ref_test(heap)
                .local_get(b)
                .ref_test(heap)
                .i32_and();
            f.if_(BlockType::Empty);
            for value in [a, b] {
                f.ins().local_get(value);
                self.narrow(m, &mut f, Rep::Uniform, Rep::Object(class));
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
        let ty = m.func_type(&[self.uniform.val_type()], &[]);
        let mut f = FuncState::new(1);
        let value = f.param(0);
        f.ins().local_get(value).ref_is_null();
        f.if_(BlockType::Empty);
        self.write(m, &mut f, Rep::None);
        f.ins().return_();
        f.end();
        for (rep, box_type) in self.uniform.boxes(m) {
            f.ins().local_get(value).ref_test(box_type);
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, rep);
            self.write(m, &mut f, rep);
            f.ins().return_();
            f.end();
        }
        let str = self.str.type_index().heap_type();
        f.ins().local_get(value).ref_test(str);
        f.if_(BlockType::Empty);
        f.ins().local_get(value);
        self.narrow(m, &mut f, Rep::Uniform, Rep::Str);
        self.str.write(&mut f);
        f.ins().return_();
        f.end();
        // The print of a union makes the `str` of each class in its type.
        for i in 0..self.classes.len() {
            let class = ClassId::new(i);
            if self.class_strs.get(i).copied().flatten().is_none() {
                continue;
            }
            f.ins()
                .local_get(value)
                .ref_test(self.class(class).heap_type());
            f.if_(BlockType::Empty);
            f.ins().local_get(value);
            self.narrow(m, &mut f, Rep::Uniform, Rep::Object(class));
            self.write(m, &mut f, Rep::Object(class));
            f.ins().return_();
            f.end();
        }
        f.ins().unreachable();
        m.add_func(ty, f.finish())
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
