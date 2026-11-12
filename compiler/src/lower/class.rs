//! The classes of the module. The compiler reads a fixed form of a
//! dataclass, of an `Enum` and of a class with an `__init__` in the text
//! and asks the checker for the names and the types. The compiled program
//! runs no code of the decorator, of the base nor of the annotations.

use std::collections::HashMap;

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange, TextSize};
use ty_module_resolver::KnownModule;
use ty_python_semantic::types::KnownClass;
use ty_python_semantic::types::lowering::{
    ArgBinding, ClassId as CheckerClass, ClassInfo, Shape, call_bindings, class_info,
    declared_shape, expr_shape,
};

use super::call::is_module;
use super::expr::int_literal;
use super::flow::{imported_name, names_in};
use super::init::{Base, check_init};
use super::{HashSet, Lowering, Scope, Sig, TypeArg, Unit, Vars, params_of, unsupported};
use crate::Error;
use crate::cpython::dataclasses::{CLASS_NAMESPACE, mutable_default, mutable_default_frames};
use crate::cpython::{Class, names};
use crate::ir::{
    Binding, ClassId, CmpOp, Expr, FuncId, Literal, LogicOp, Place, Rep, Site, Stmt, UserClass,
    VarId,
};

/// The classes of the module.
#[derive(Default)]
pub(super) struct Classes<'a> {
    /// The class that each name of the module binds, the first one for a
    /// name with more than one `class`.
    pub names: HashMap<String, ClassId>,
    /// The class of each class of the checker.
    pub ids: HashMap<CheckerClass, ClassId>,
    pub list: Vec<Declared<'a>>,
    /// The classes whose objects the program makes, in a call of the class
    /// or as the members of an `Enum`.
    pub instantiated: HashSet<ClassId>,
    /// The calls of a method through a value of a class, which wait for
    /// the classes that the program makes.
    pub slots: Vec<Slot>,
    /// The place of each slot in `slots`.
    pub slot_ids: HashMap<(ClassId, Key), usize>,
    /// The number of slots that [`Lowering::resolve_slots`] has defined,
    /// which are the first ones of `slots`.
    pub resolved: usize,
    /// The copy of each generic class of the prelude, by the file and the
    /// name of the class and what each of its type variables stands for.
    pub copies: HashMap<(File, String, Vec<TypeArg>), ClassId>,
}

/// What a call through a value of a class calls.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Key {
    /// A method of the program, by its name.
    Method(String),
    /// The `str` of the object.
    Str,
    /// The `repr` of the object.
    Repr,
    /// The truth of the object.
    Truth,
}

/// The function that a call of `key` through a value of `class` calls,
/// which [`Lowering::resolve_slots`] defines once the lowering knows the
/// classes that the program makes.
pub(super) struct Slot {
    class: ClassId,
    key: Key,
    func: FuncId,
}

impl<'a> Classes<'a> {
    pub fn get(&self, id: ClassId) -> &Declared<'a> {
        self.list.get(id.index()).expect("a class of the module")
    }

    pub fn get_mut(&mut self, id: ClassId) -> &mut Declared<'a> {
        self.list
            .get_mut(id.index())
            .expect("a class of the module")
    }

    /// Returns `true` if an object of `class` has the attribute `name`
    /// once its `__init__` returns, `false` otherwise.
    pub fn has_attribute(&self, class: ClassId, name: &str) -> bool {
        let declared = self.get(class);
        declared.fields.iter().any(|(field, _)| field == name)
            || self
                .chain(class)
                .any(|class| self.own_method(class, name).is_some())
            || (name == "__annotations__" && !declared.fields.is_empty())
            || names::OBJECT_ATTRIBUTES.binary_search(&name).is_ok()
            || (declared.kind.is_dataclass()
                && names::DATACLASS_ATTRIBUTES.binary_search(&name).is_ok())
    }

    /// The method `name` that the body of `class` defines.
    pub fn own_method(&self, class: ClassId, name: &str) -> Option<Method<'a>> {
        self.get(class)
            .methods
            .iter()
            .find(|method| method.def.name.as_str() == name)
            .copied()
    }

    /// `class` and then each class that it descends from, in order.
    pub fn chain(&self, class: ClassId) -> impl Iterator<Item = ClassId> {
        std::iter::successors(Some(class), |class| self.get(*class).base)
    }

    /// The classes that are `class` or descend from it, in the order of
    /// their ids.
    pub fn descendants(&self, class: ClassId) -> Vec<ClassId> {
        (0..self.list.len())
            .map(ClassId::new)
            .filter(|other| self.chain(*other).any(|c| c == class))
            .collect()
    }

    /// The classes of the IR.
    pub fn into_ir(self) -> Vec<UserClass> {
        let slot = |id: usize, key: Key| {
            let slot = *self.slot_ids.get(&(ClassId::new(id), key))?;
            Some(self.slots.get(slot).expect("a slot").func)
        };
        self.list
            .into_iter()
            .enumerate()
            .map(|(id, class)| UserClass {
                ordered: class.ordered,
                base: class.base,
                fields: class.fields.into_iter().map(|(_, rep)| rep).collect(),
                mutable: class.kind.is_assignable(),
                unset: class.kind == Kind::Plain,
                members: class
                    .members
                    .into_iter()
                    .map(|member| vec![Literal::Str(member.name), member.value])
                    .collect(),
                str: slot(id, Key::Str),
                repr: slot(id, Key::Repr),
                repr_guard: class.kind.is_dataclass(),
            })
            .collect()
    }
}

/// The name and the type of each field of a class, as the checker gives
/// them.
type Checked = Vec<(String, Shape)>;

/// A class of the module, with what the lowering knows of it.
pub(super) struct Declared<'a> {
    pub def: &'a ast::StmtClassDef,
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
    pub kind: Kind,
    /// The class of the program that the class extends.
    pub base: Option<ClassId>,
    /// `true` when the `__init__` of a class of [`Kind::Plain`], its own
    /// or the one that it takes, lets `self` escape.
    pub init_escapes: bool,
    /// The fields that the body of a dataclass or of a class of
    /// [`Kind::Plain`] annotates, in the order of the body.
    pub checked: Checked,
    /// The node of the annotation and the type of each field, in the
    /// order of `fields`.
    pub nodes: Vec<(&'a ast::StmtAnnAssign, Shape)>,
    /// The name and the rep of each field of an object of the class. A
    /// member of an `Enum` has `name` and `value`.
    pub fields: Vec<(String, Rep)>,
    pub members: Vec<Member>,
    /// The `str` that the compiler writes for an object of exactly the
    /// class, as the one of a member of an `Enum`.
    pub own_str: Option<FuncId>,
    /// The `repr` that the compiler writes for an object of exactly the
    /// class, and that a class without `__repr__` has.
    pub own_repr: Option<FuncId>,
    /// The truth that the compiler writes for an object of a class whose
    /// truth comes from the `__len__` of this class, or, for a class
    /// without a base, from neither `__bool__` nor `__len__`.
    pub own_truth: Option<FuncId>,
    /// The function that makes an object from a value for each field of a
    /// dataclass, or for each parameter of the `__init__` of a class of
    /// [`Kind::Plain`], once a call needs it.
    pub init: Option<FuncId>,
    /// The `__init__` of a dataclass, which assigns each field of an object
    /// of the class or of a subclass, once a call of `super().__init__`
    /// needs it.
    pub reinit: Option<FuncId>,
    /// The methods of the class, in the order of the body.
    pub methods: Vec<Method<'a>>,
    /// The function `__eq__` of a dataclass, once a comparison needs it.
    pub eq: Option<FuncId>,
    /// The function that compares two values of a dataclass that may be
    /// `None`, once a comparison needs it.
    pub eq_or_none: Option<FuncId>,
    /// The function that gives the `str` of an object of the class or
    /// `None`, once a conversion needs it.
    pub str_or_none: Option<FuncId>,
    /// The function that gives the `repr` of an object of the class or
    /// `None`, once a conversion needs it.
    pub repr_or_none: Option<FuncId>,
    /// The variable of the module that holds the default of each field of
    /// a dataclass that has one. The `class` statement gives it its value.
    pub defaults: Vec<Option<VarId>>,
    /// What the class copies, for a copy of a generic class of the prelude.
    pub copy: Option<ClassCopy>,
}

/// A generic class of the prelude, in the file of its module, with what
/// each of its type variables stands for in a copy.
#[derive(Clone)]
pub(super) struct ClassCopy {
    pub file: File,
    pub types: Vec<(String, TypeArg)>,
}

impl Declared<'_> {
    /// The function that gives the `repr` of an object or `None` when
    /// `repr` is `true`, and the one that gives the `str` otherwise.
    fn or_none(&mut self, repr: bool) -> &mut Option<FuncId> {
        if repr {
            &mut self.repr_or_none
        } else {
            &mut self.str_or_none
        }
    }
}

/// The forms of a class that the compiler takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Enum,
    /// `frozen` is `true` for `frozen=True`, whose fields keep their first
    /// value.
    Dataclass {
        frozen: bool,
    },
    /// A class without a decorator, with annotated fields that its
    /// `__init__` assigns, and with a base of the same form or none.
    Plain,
}

impl Kind {
    pub fn is_dataclass(self) -> bool {
        matches!(self, Kind::Dataclass { .. })
    }

    /// Returns `true` if a program may assign the fields of an object of
    /// the class, `false` otherwise.
    fn is_assignable(self) -> bool {
        matches!(self, Kind::Dataclass { frozen: false } | Kind::Plain)
    }
}

/// A method of a class, with its function once
/// [`Lowering::declare_methods`] declares it.
#[derive(Clone, Copy)]
pub(super) struct Method<'a> {
    pub def: &'a ast::StmtFunctionDef,
    pub func: Option<FuncId>,
}

impl Method<'_> {
    fn func(&self) -> FuncId {
        self.func.expect("a declared method")
    }
}

/// A member of an `Enum`, with its value.
pub(super) struct Member {
    pub name: String,
    pub value: Literal,
}

impl<'a, 'db> Lowering<'a, 'db> {
    /// Declares the class of `def`, a `class` statement of the module. The
    /// fields of a dataclass wait for [`Self::declare_fields`], since they
    /// may hold an object of a class that comes later.
    pub(super) fn declare_class(&self, def: &'a ast::StmtClassDef) -> Result<(), Error> {
        let name = def.name.as_str();
        if self.imports.borrow().ids.contains_key(name) {
            return Err(unsupported(&def.name, "a name of an import and of a class"));
        }
        // A decorator other than `dataclass` may make the class something
        // else, which the checker does not describe.
        let frozen = (!def.decorator_list.is_empty())
            .then(|| self.dataclass_decorator(def))
            .transpose()?;
        let info = class_info(self.model(), def).ok_or_else(|| unsupported(def, "this class"))?;
        let base = self.program_base(def, &info, frozen)?;
        let mut init_escapes = false;
        let (kind, checked, fields, members, methods) = if let Some(frozen) = frozen {
            let (checked, methods) = self.dataclass_form(def, &info, base)?;
            let kind = Kind::Dataclass { frozen };
            (kind, checked, Vec::new(), Vec::new(), methods)
        } else if base.is_some() || (def.arguments.is_none() && info.mro.len() == 2) {
            let (checked, methods, escapes) = self.plain_form(def, &info, base)?;
            init_escapes = escapes;
            (Kind::Plain, checked, Vec::new(), Vec::new(), methods)
        } else {
            let (members, methods) =
                self.enum_members(def, &info.mro, info.enum_members.as_deref())?;
            let value = members
                .first()
                .map_or(Rep::Int, |member| member.value.rep());
            let fields = vec![("name".to_owned(), Rep::Str), ("value".to_owned(), value)];
            (Kind::Enum, Vec::new(), fields, members, methods)
        };
        let mut classes = self.classes.borrow_mut();
        let id = ClassId::new(classes.list.len());
        classes.list.push(Declared {
            def,
            ordered: false,
            kind,
            base,
            init_escapes,
            checked,
            nodes: Vec::new(),
            fields,
            members,
            own_str: None,
            own_repr: None,
            own_truth: None,
            init: None,
            reinit: None,
            methods,
            eq: None,
            eq_or_none: None,
            str_or_none: None,
            repr_or_none: None,
            defaults: Vec::new(),
            copy: None,
        });
        if kind == Kind::Enum && !classes.get(id).members.is_empty() {
            classes.instantiated.insert(id);
        }
        // The first `class` of a name runs before the others, since a class
        // in an `if` or a `while` is unsupported, so the name has a value
        // once the first one has.
        classes.names.entry(name.to_owned()).or_insert(id);
        classes.ids.insert(info.id, id);
        Ok(())
    }

    /// The copy of the generic class `name` of `unit`, a module of the
    /// prelude, whose type variables stand for what `args` gives in order,
    /// which the first use declares. `None` when the module has no such
    /// class, or a type variable has no rep. A call declares each method.
    pub(super) fn prelude_class(
        &self,
        unit: &'a Unit<'a, 'db>,
        name: &str,
        args: Vec<Shape>,
    ) -> Option<ClassId> {
        let def = *unit.classes.get(name)?;
        let args = args
            .into_iter()
            .map(|shape| self.type_arg(shape))
            .collect::<Option<Vec<_>>>()?;
        let key = (unit.file, name.to_owned(), args.clone());
        if let Some(&class) = self.classes.borrow().copies.get(&key) {
            return Some(class);
        }
        let types: Vec<(String, TypeArg)> = def
            .type_params
            .iter()
            .flat_map(|params| &params.type_params)
            .map(|param| param.name().to_string())
            .zip(args)
            .collect();
        let outer = self.unit.replace(unit);
        let outer_types = self.types.replace(types.clone());
        let form = self.copy_form(def);
        self.types.replace(outer_types);
        self.unit.set(outer);
        let (checked, methods, nodes, fields) = form;
        let mut classes = self.classes.borrow_mut();
        let id = ClassId::new(classes.list.len());
        classes.list.push(Declared {
            def,
            ordered: false,
            kind: Kind::Plain,
            base: None,
            init_escapes: false,
            checked,
            nodes,
            fields,
            members: Vec::new(),
            own_str: None,
            own_repr: None,
            own_truth: None,
            init: None,
            reinit: None,
            methods,
            eq: None,
            eq_or_none: None,
            str_or_none: None,
            repr_or_none: None,
            defaults: Vec::new(),
            copy: Some(ClassCopy {
                file: unit.file,
                types,
            }),
        });
        classes.copies.insert(key, id);
        Some(id)
    }

    /// The fields and the methods of `def`, a generic class of the module
    /// of the prelude that the lowering is in, with the types of the copy
    /// that the lowering declares. Each field has the rep of its type,
    /// which a list of a type variable always has.
    #[expect(clippy::type_complexity)]
    fn copy_form(
        &self,
        def: &'a ast::StmtClassDef,
    ) -> (
        Checked,
        Vec<Method<'a>>,
        Vec<(&'a ast::StmtAnnAssign, Shape)>,
        Vec<(String, Rep)>,
    ) {
        let info = class_info(self.model(), def).expect("a class of the prelude");
        let (checked, methods, _) = self
            .plain_form(def, &info, None)
            .expect("a class of the prelude of the plain form");
        let nodes = dataclass_fields(def)
            .zip(checked.iter().map(|(_, shape)| shape.clone()))
            .collect();
        let fields = checked
            .iter()
            .map(|(name, shape)| {
                let rep = self
                    .rep_of(Some(shape.clone()))
                    .expect("a field of a class of the prelude with a rep");
                (name.clone(), rep)
            })
            .collect();
        (checked, methods, nodes, fields)
    }

    /// Gives each field of each class of the module with fields that the
    /// checker gives the rep of its values, once every class of the module
    /// has an id.
    pub(super) fn declare_fields(&self) -> Result<(), Error> {
        let count = self.classes.borrow().list.len();
        for class in (0..count).map(ClassId::new) {
            let (def, checked, base) = {
                let classes = self.classes.borrow();
                let declared = classes.get(class);
                if declared.kind == Kind::Enum || declared.copy.is_some() {
                    continue;
                }
                let base = declared.base.map(|base| {
                    let base = classes.get(base);
                    (
                        base.fields.clone(),
                        base.defaults.clone(),
                        base.nodes.clone(),
                    )
                });
                (declared.def, declared.checked.clone(), base)
            };
            // The fields of the base come first, and a field that the class
            // declares again keeps its place, with the default of the class.
            let (mut fields, mut defaults, mut nodes) = base.unwrap_or_default();
            for ((name, shape), node) in checked.into_iter().zip(dataclass_fields(def)) {
                let rep = self
                    .rep_of(Some(shape.clone()))
                    .filter(|rep| *rep != Rep::None)
                    .ok_or_else(|| unsupported(node, "a field of this type"))?;
                let default = node
                    .value
                    .as_ref()
                    .map(|_| self.vars.borrow_mut().add_unnamed(rep));
                match fields.iter().position(|(field, _)| *field == name) {
                    Some(i) => {
                        let inherited = fields.get(i).expect("a field of the base").1;
                        // The checker rejects a field of another type, which
                        // the struct of a subclass cannot have.
                        if inherited != rep {
                            return Err(unsupported(
                                node,
                                "a field of another type than in the base",
                            ));
                        }
                        *defaults.get_mut(i).expect("a field of the base") = default;
                        *nodes.get_mut(i).expect("a field of the base") = (node, shape);
                    }
                    None => {
                        fields.push((name, rep));
                        defaults.push(default);
                        nodes.push((node, shape));
                    }
                }
            }
            let mut classes = self.classes.borrow_mut();
            let declared = classes.get_mut(class);
            declared.fields = fields;
            declared.defaults = defaults;
            declared.nodes = nodes;
        }
        Ok(())
    }

    /// Declares each method of each class, once each field has its rep.
    pub(super) fn declare_methods(&self) -> Result<(), Error> {
        let count = self.classes.borrow().list.len();
        for class in (0..count).map(ClassId::new) {
            // A call declares a method of a copy, with the types of the copy.
            if self.classes.borrow().get(class).copy.is_some() {
                continue;
            }
            let mut methods = self.classes.borrow().get(class).methods.clone();
            for method in &mut methods {
                method.func = Some(self.declare_method(method.def, class)?);
            }
            self.classes.borrow_mut().get_mut(class).methods = methods.clone();
            for method in &methods {
                self.check_special(class, *method)?;
            }
            let base = self.classes.borrow().get(class).base;
            for method in methods
                .iter()
                .filter(|method| method.def.name.as_str() != "__init__")
            {
                if let Some(overridden) = base.and_then(|base| self.method(base, &method.def.name))
                {
                    self.check_override(*method, overridden)?;
                }
            }
        }
        Ok(())
    }

    /// Checks that `method` may take the place of `overridden`, a method of
    /// a class that the class of `method` descends from, in a call through
    /// a value of that class. The call binds the arguments by the
    /// parameters of `overridden`, so both have the same parameters, of
    /// the same reps, without defaults, whose values CPython takes from
    /// the method that runs.
    fn check_override(&self, method: Method<'a>, overridden: Method<'a>) -> Result<(), Error> {
        let funcs = self.funcs.borrow();
        let (ours, theirs) = (funcs.get(method.func()), funcs.get(overridden.func()));
        let names = |def: &ast::StmtFunctionDef| {
            params_of(def)
                .skip(1)
                .map(|param| param.name().to_string())
                .collect::<Vec<_>>()
        };
        let defaults =
            |def: &ast::StmtFunctionDef| params_of(def).any(|param| param.default.is_some());
        if defaults(method.def) || defaults(overridden.def) {
            return Err(unsupported(
                method.def,
                "an override of a method with defaults",
            ));
        }
        let ret = ours.ret == theirs.ret
            || (ours.ret.class().is_some() && self.holds(theirs.ret, ours.ret));
        if names(method.def) != names(overridden.def)
            || ours.params.get(1..) != theirs.params.get(1..)
            || !ret
        {
            return Err(unsupported(
                method.def,
                "an override with other parameters or result",
            ));
        }
        Ok(())
    }

    /// Refuses a special method of `class` whose parameters and result
    /// differ from the ones that the operator gives and takes. `__eq__`
    /// and `__ne__` take any value, and the methods of an order take one
    /// value, and each of them gives a `bool`. The method of an arithmetic
    /// operator takes one value, and `__neg__`, `__len__` and `__bool__`
    /// take none. `__len__` gives an `int`, and `__bool__` a `bool`.
    /// `__contains__` takes a value and gives a `bool`, `__getitem__`
    /// takes a key, and `__setitem__` takes a key and a value and gives
    /// `None`.
    fn check_special(&self, class: ClassId, method: Method<'a>) -> Result<(), Error> {
        let name = method.def.name.as_str();
        let funcs = self.funcs.borrow();
        let sig = funcs.get(method.func());
        let defaults = params_of(method.def).any(|param| param.default.is_some());
        let unary = match name {
            "__neg__" => Some(None),
            "__len__" => Some(Some(Rep::Int)),
            "__bool__" => Some(Some(Rep::Bool)),
            _ => None,
        };
        if let Some(ret) = unary {
            if defaults || sig.params.len() != 1 || ret.is_some_and(|ret| sig.ret != ret) {
                return Err(unsupported(
                    method.def,
                    "a __neg__, __len__ or __bool__ other than (self)",
                ));
            }
            return Ok(());
        }
        let other = match name {
            "__eq__" | "__ne__" => Some(Rep::Uniform),
            "__lt__" | "__le__" | "__gt__" | "__ge__" => None,
            "__contains__" | "__getitem__" | "__setitem__" => {
                let (count, ret) = match name {
                    "__contains__" => (2, Some(Rep::Bool)),
                    "__getitem__" => (2, None),
                    _ => (3, Some(Rep::None)),
                };
                if defaults || sig.params.len() != count || ret.is_some_and(|ret| sig.ret != ret) {
                    return Err(unsupported(
                        method.def,
                        "a __contains__, __getitem__ or __setitem__ of another form",
                    ));
                }
                return Ok(());
            }
            _ if COMPILED_SPECIAL.contains(&name) => {
                if defaults || sig.params.len() != 2 {
                    return Err(unsupported(
                        method.def,
                        "a method of an operator other than (self, other)",
                    ));
                }
                return Ok(());
            }
            _ => return Ok(()),
        };
        let params = match (&*sig.params, other) {
            ([object, _], None) => *object == Rep::Object(class),
            ([object, param], Some(other)) => *object == Rep::Object(class) && *param == other,
            _ => false,
        };
        if defaults || !params || sig.ret != Rep::Bool {
            let what = match other {
                Some(_) => "an __eq__ or __ne__ other than (self, other: object) -> bool",
                None => "a method of an order other than (self, other) -> bool",
            };
            return Err(unsupported(method.def, what));
        }
        Ok(())
    }

    /// Declares `def`, a method of `class`.
    fn declare_method(
        &self,
        def: &'a ast::StmtFunctionDef,
        class: ClassId,
    ) -> Result<FuncId, Error> {
        let (id, params, ret) = self.signature(def, Some(class))?;
        let defaults = {
            let mut vars = self.vars.borrow_mut();
            params_of(def)
                .zip(&params)
                .map(|(param, rep)| param.default.as_ref().map(|_| vars.add_unnamed(*rep)))
                .collect()
        };
        Ok(self.funcs.borrow_mut().add(
            id,
            Sig {
                unit: self.unit.get(),
                def: Some(def),
                params,
                defaults,
                doctest: None,
                ret,
                types: Vec::new(),
            },
        ))
    }

    /// The method `name` of `class`, which the class defines or takes from
    /// the nearest class that it descends from, and `None` when no such
    /// class has a method of the name.
    pub(super) fn method(&self, class: ClassId, name: &str) -> Option<Method<'a>> {
        let (owner, method) = {
            let classes = self.classes.borrow();
            classes
                .chain(class)
                .find_map(|class| Some((class, classes.own_method(class, name)?)))?
        };
        let copy = self.classes.borrow().get(owner).copy.clone();
        match copy {
            Some(copy) if method.func.is_none() => Some(Method {
                func: Some(self.declare_copy_method(owner, copy, method.def)),
                ..method
            }),
            _ => Some(method),
        }
    }

    /// Declares `def`, a method of `class`, a copy of a generic class of
    /// the prelude, with the types of the copy. A parameter with a default
    /// holds `None` for it, as in a function of the prelude.
    fn declare_copy_method(
        &self,
        class: ClassId,
        copy: ClassCopy,
        def: &'a ast::StmtFunctionDef,
    ) -> FuncId {
        let unit = self.prelude_unit_of(copy.file);
        let outer = self.unit.replace(unit);
        let outer_types = self.types.replace(copy.types.clone());
        let signature = self.signature(def, Some(class));
        self.types.replace(outer_types);
        self.unit.set(outer);
        // The prelude joins a type variable only with `None`, which every
        // rep joins.
        let (_, params, ret) = signature.expect("a method of the prelude with a signature");
        let sig = Sig {
            unit,
            def: Some(def),
            defaults: params.iter().map(|_| None).collect(),
            doctest: None,
            params,
            ret,
            types: copy.types,
        };
        let func = self.funcs.borrow_mut().add_generated(sig, true);
        let mut classes = self.classes.borrow_mut();
        let method = classes
            .get_mut(class)
            .methods
            .iter_mut()
            .find(|method| std::ptr::eq(method.def, def))
            .expect("a method of the class");
        method.func = Some(func);
        func
    }

    /// The file of the generic class of the prelude that `class` copies,
    /// and `None` for a class of the program.
    pub(super) fn copy_file(&self, class: ClassId) -> Option<File> {
        Some(self.classes.borrow().get(class).copy.as_ref()?.file)
    }

    /// The function of [`Self::method`], once the lowering declares it.
    pub(super) fn method_func(&self, class: ClassId, name: &str) -> Option<FuncId> {
        self.method(class, name)?.func
    }

    /// The classes that are `class` or descend from it.
    pub(super) fn descendants(&self, class: ClassId) -> Vec<ClassId> {
        self.classes.borrow().descendants(class)
    }

    /// Returns `true` if `class` is `ancestor` or descends from it, `false`
    /// otherwise.
    pub(super) fn descends(&self, class: ClassId, ancestor: ClassId) -> bool {
        self.classes.borrow().chain(class).any(|c| c == ancestor)
    }

    /// The class of `def`, a `class` statement that the lowering finds in
    /// the code.
    pub(super) fn class_of(&self, def: &ast::StmtClassDef) -> Result<ClassId, Error> {
        if self.scope.borrow().is_some() {
            return Err(unsupported(def, "a class in a function"));
        }
        let classes = self.classes.borrow();
        classes
            .list
            .iter()
            .position(|declared| std::ptr::eq(declared.def, def))
            .map(ClassId::new)
            .ok_or_else(|| unsupported(def, "a class in an if or a while"))
    }

    /// The code that the `class` statement of `def` runs before it binds
    /// the name. The body of a dataclass evaluates the defaults, in a frame
    /// with the name of the class. The decorator then raises `ValueError`
    /// for the first default that is an object of a dataclass.
    pub(super) fn class_body(&self, def: &ast::StmtClassDef) -> Result<Vec<Stmt>, Error> {
        let class = self.class_of(def)?;
        let (kind, fields, defaults) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (
                declared.kind,
                declared.fields.clone(),
                declared.defaults.clone(),
            )
        };
        let mut namespace: Vec<String> = CLASS_NAMESPACE.map(str::to_owned).into();
        if kind == Kind::Enum {
            namespace.push("_generate_next_value_".to_owned());
        }
        if def.body.iter().any(ast::Stmt::is_ann_assign_stmt) {
            namespace.push("__annotations__".to_owned());
        }
        if def.body.first().is_some_and(
            |stmt| matches!(stmt, ast::Stmt::Expr(e) if e.value.is_string_literal_expr()),
        ) {
            namespace.push("__doc__".to_owned());
        }
        *self.scope.borrow_mut() = Some(Scope {
            params: 0,
            doctest: None,
            name: def.name.to_string(),
            ret: Rep::None,
            locals: HashSet::new(),
            order: Vec::new(),
            vars: Vars::default(),
            namespace,
            class: Some(def.start()),
        });
        let body = self.class_statements(def, class, &fields, &defaults);
        let scope = self.scope.take().expect("the scope of the class");
        let (body, mutable) = body?;
        // The body of a class reads no variable of its own.
        debug_assert!(scope.vars.list.is_empty(), "a variable in a class");
        let mut stmts = Vec::new();
        if !body.is_empty() {
            let func = self.reserve(Vec::new(), Rep::None);
            self.define(func, true, body);
            // CPython shows the statement from the keyword on, without the
            // decorator.
            let start = self.source()[..usize::from(def.name.start())]
                .rfind("class")
                .expect("the keyword of a class");
            let range = TextRange::new(
                TextSize::try_from(start).expect("a file shorter than 4 GiB"),
                def.end(),
            );
            stmts.push(Stmt::Expr(generated_call(
                func,
                Vec::new(),
                Rep::None,
                self.site(range),
            )));
        }
        stmts.extend(mutable);
        Ok(stmts)
    }

    /// The code of the body of `def`, the class `class` with `fields` and
    /// `defaults`, in the scope of the body. The body evaluates the default
    /// and the annotation of each field, and the `def` of each method
    /// evaluates the defaults and the annotations of its parameters. The
    /// second value is the raise of the decorator of a dataclass for the
    /// first default that is an object of a mutable dataclass.
    fn class_statements(
        &self,
        def: &ast::StmtClassDef,
        class: ClassId,
        fields: &[(String, Rep)],
        defaults: &[Option<VarId>],
    ) -> Result<(Vec<Stmt>, Option<Stmt>), Error> {
        let mut body = Vec::new();
        let mut mutable = None;
        for stmt in &def.body {
            match stmt {
                // A member of an `Enum` joins the namespace.
                ast::Stmt::Assign(member) => {
                    if let [ast::Expr::Name(name)] = &*member.targets {
                        self.scope
                            .borrow_mut()
                            .as_mut()
                            .expect("the scope of the class body")
                            .namespace
                            .push(name.id.to_string());
                    }
                }
                ast::Stmt::AnnAssign(node) => {
                    let target = node.target.as_name_expr().expect("a name").id.as_str();
                    let i = fields
                        .iter()
                        .position(|(name, _)| name == target)
                        .expect("a field of the class");
                    let (name, rep) = fields.get(i).expect("a field of the class");
                    let var = defaults.get(i).expect("a field of the class");
                    if let (Some(value), Some(var)) = (node.value.as_deref(), var) {
                        body.push(self.class_default(
                            def,
                            value,
                            name,
                            *rep,
                            *var,
                            &mut mutable,
                        )?);
                    }
                    self.check_annotations(std::iter::once(&*node.annotation))?;
                }
                ast::Stmt::FunctionDef(method) => {
                    let func = self
                        .method(class, method.name.as_str())
                        .expect("a method of the class")
                        .func();
                    body.extend(self.default_values(method, func)?);
                    let annotations = params_of(method)
                        .filter_map(|param| param.annotation())
                        .chain(method.returns.as_deref());
                    self.check_annotations(annotations)?;
                    self.scope
                        .borrow_mut()
                        .as_mut()
                        .expect("the scope of the class body")
                        .namespace
                        .push(method.name.to_string());
                }
                _ => {}
            }
        }
        Ok((body, mutable))
    }

    /// The assignment of `value`, the default of the field `name` of `def`,
    /// to `var`, which holds values of `rep`. `mutable` gets the raise of
    /// the decorator when the default is the first that is a list or an
    /// object of a mutable dataclass.
    fn class_default(
        &self,
        def: &ast::StmtClassDef,
        value: &ast::Expr,
        name: &str,
        rep: Rep,
        var: VarId,
        mutable: &mut Option<Stmt>,
    ) -> Result<Stmt, Error> {
        let lowered = self.expr(value)?;
        let object = match lowered.rep() {
            Rep::Object(class) if self.is_mutable(class) => Some(format!(
                "__main__.{}",
                self.classes.borrow().get(class).def.name
            )),
            Rep::List(_) => Some("list".to_owned()),
            Rep::ObjectOrNone(_) | Rep::ListOrNone(_) | Rep::Uniform
                if self.may_be_mutable(expr_shape(self.model(), value)) =>
            {
                return Err(unsupported(
                    value,
                    "a default that may be a list or an object of a dataclass",
                ));
            }
            _ => None,
        };
        if let Some(object) = object.filter(|_| mutable.is_none()) {
            let decorator = &def
                .decorator_list
                .first()
                .expect("the decorator of a dataclass")
                .expression;
            let frames = mutable_default_frames(decorator.is_call_expr());
            *mutable = Some(Stmt::Raise {
                class: Class::ValueError,
                arg: Some(Expr::Str(mutable_default(&object, name))),
                site: self.site_with(decorator.range(), &frames),
            });
        }
        let lowered = self
            .coerce(lowered, rep)
            .ok_or_else(|| unsupported(value, "a default of this type"))?;
        self.scope
            .borrow_mut()
            .as_mut()
            .expect("the scope of the class body")
            .namespace
            .push(name.to_owned());
        Ok(Stmt::Assign {
            targets: vec![Place::Global(var)],
            value: lowered,
        })
    }

    /// Returns `true` if a value of `shape` may be a list or an object of a
    /// class that [`Self::is_mutable`] accepts, `false` otherwise.
    fn may_be_mutable(&self, shape: Option<Shape>) -> bool {
        let list = match &shape {
            Some(Shape::List(_)) => true,
            Some(Shape::Union(shapes)) => {
                shapes.iter().any(|shape| matches!(shape, Shape::List(_)))
            }
            _ => false,
        };
        list || self
            .classes_in(shape)
            .into_iter()
            .any(|class| self.is_mutable(class))
    }

    /// Returns `true` if `class` is a dataclass whose fields a program may
    /// assign, `false` otherwise. CPython calls a default of such a class
    /// mutable, because the `__eq__` of the class takes its hash away.
    fn is_mutable(&self, class: ClassId) -> bool {
        self.classes.borrow().get(class).kind == Kind::Dataclass { frozen: false }
    }

    /// The value of `attr`, an attribute of a class, which has to be a
    /// member of an `Enum`.
    pub(super) fn class_attribute(
        &self,
        attr: &ast::ExprAttribute,
        name: &ast::ExprName,
        class: ClassId,
    ) -> Result<Expr, Error> {
        let member = self
            .classes
            .borrow()
            .get(class)
            .members
            .iter()
            .position(|member| member.name == attr.attr.as_str())
            .ok_or_else(|| unsupported(attr, "this attribute of a class"))?;
        let member = Expr::Member {
            class,
            member: u32::try_from(member).expect("fewer than 2^32 members"),
        };
        self.after(Binding::Class(class), name, member)
    }

    /// The value of `attr`, an attribute of an object of `class`, which
    /// `object` gives. A member of an `Enum` has `name` and `value`, and an
    /// object of a dataclass has its fields.
    pub(super) fn object_attribute(
        &self,
        attr: &ast::ExprAttribute,
        object: Expr,
        class: ClassId,
    ) -> Result<Expr, Error> {
        let (field, rep) = self.field_of(attr, class)?;
        let text = format!("'NoneType' object has no attribute '{}'", attr.attr);
        Ok(Expr::Field {
            object: Box::new(self.not_none(object, text, attr)),
            class,
            field,
            rep,
        })
    }

    /// `object`, which raises `AttributeError` with `text` at the site of
    /// `attr` when it is `None`. A member of an `Enum` is never `None`.
    pub(super) fn not_none(&self, object: Expr, text: String, attr: &ast::ExprAttribute) -> Expr {
        if matches!(object, Expr::Member { .. }) {
            return object;
        }
        Expr::NotNone {
            value: Box::new(object),
            text,
            site: self.site(attr.range()),
        }
    }

    /// The assignment of `value` to `attr`, a field of an object of a
    /// dataclass.
    pub(super) fn assign_field(
        &self,
        attr: &ast::ExprAttribute,
        value: Expr,
    ) -> Result<Stmt, Error> {
        let (object, class, field, rep) = self.updated_field(attr)?;
        let value = self
            .coerce(value, rep)
            .ok_or_else(|| unsupported(attr, "a field with values of two types"))?;
        let text = format!(
            "'NoneType' object has no attribute '{}' and no __dict__ for setting new attributes",
            attr.attr
        );
        Ok(Stmt::SetField {
            object: self.not_none(object, text, attr),
            class,
            field,
            value,
        })
    }

    /// The object of `attr`, a field that a statement assigns, together
    /// with the class of the object, the index of the field and its rep.
    pub(super) fn updated_field(
        &self,
        attr: &ast::ExprAttribute,
    ) -> Result<(Expr, ClassId, u32, Rep), Error> {
        // The checker rejects an assignment to a field of a frozen
        // dataclass, whose fields the struct keeps immutable.
        let class = match self.rep_of(expr_shape(self.model(), &attr.value)) {
            Some(Rep::Object(class)) if self.classes.borrow().get(class).kind.is_assignable() => {
                class
            }
            _ => return Err(unsupported(attr, "this target")),
        };
        let object = self.expr(&attr.value)?;
        let (field, rep) = self.field_of(attr, class)?;
        Ok((object, class, field, rep))
    }

    /// The index and the rep of the field of `class` that `attr` names.
    fn field_of(&self, attr: &ast::ExprAttribute, class: ClassId) -> Result<(u32, Rep), Error> {
        let classes = self.classes.borrow();
        let declared = classes.get(class);
        // The fields of a copy are the data of a builtin of CPython, which
        // only the code of the prelude reads.
        if declared.copy.is_some() && !self.in_prelude() {
            return Err(unsupported(attr, "this attribute of an object"));
        }
        let (field, (_, rep)) = declared
            .fields
            .iter()
            .enumerate()
            .find(|(_, (name, _))| *name == attr.attr.as_str())
            .ok_or_else(|| unsupported(attr, "this attribute of an object"))?;
        Ok((u32::try_from(field).expect("fewer than 2^32 fields"), *rep))
    }

    /// The function that gives the `repr` of a value of `class` when
    /// `repr` is `true`, and the one that gives the `str` otherwise. An
    /// object of a subclass may have its own.
    pub(super) fn class_conversion(&self, class: ClassId, repr: bool) -> FuncId {
        let key = if repr { Key::Repr } else { Key::Str };
        self.slot(class, key, vec![Rep::Object(class)], Rep::Str, Vec::new())
    }

    /// The function that gives the truth of a value of `class`. An object
    /// of a subclass may have its own.
    pub(super) fn class_truth(&self, class: ClassId) -> FuncId {
        self.slot(
            class,
            Key::Truth,
            vec![Rep::Object(class)],
            Rep::Bool,
            Vec::new(),
        )
    }

    /// The function that a call of `key` through a value of `class` calls,
    /// with a parameter of each rep of `params`, the first an object of
    /// `class`, the global of each default in `defaults`, and a result of
    /// `ret`. The first call reserves it.
    fn slot(
        &self,
        class: ClassId,
        key: Key,
        params: Vec<Rep>,
        ret: Rep,
        defaults: Vec<Option<VarId>>,
    ) -> FuncId {
        if let Some(&slot) = self.classes.borrow().slot_ids.get(&(class, key.clone())) {
            return self.classes.borrow().slots.get(slot).expect("a slot").func;
        }
        let defaults = if defaults.is_empty() {
            params.iter().map(|_| None).collect()
        } else {
            defaults
        };
        let func = self.reserve_with(params, ret, defaults, true);
        let mut classes = self.classes.borrow_mut();
        let slot = classes.slots.len();
        classes.slot_ids.insert((class, key.clone()), slot);
        classes.slots.push(Slot { class, key, func });
        func
    }

    /// Defines the function of each slot that waits, with the function
    /// that each class that the program makes has for the call, and
    /// returns `true` if a slot waited. A slot without a class gives code
    /// that never runs, a slot of one function calls it in its place, and
    /// a slot of more functions tests the exact class of the object.
    pub(super) fn resolve_slots(&self) -> Result<bool, Error> {
        let start = self.classes.borrow().resolved;
        loop {
            let next = {
                let mut classes = self.classes.borrow_mut();
                let next = classes
                    .slots
                    .get(classes.resolved)
                    .map(|slot| (slot.class, slot.key.clone(), slot.func));
                classes.resolved += usize::from(next.is_some());
                next
            };
            let Some((class, key, func)) = next else {
                break;
            };
            let made: Vec<ClassId> = {
                let classes = self.classes.borrow();
                classes
                    .descendants(class)
                    .into_iter()
                    .filter(|class| classes.instantiated.contains(class))
                    .collect()
            };
            let arms = made
                .into_iter()
                .map(|exact| {
                    let body = match &key {
                        Key::Method(name) => self
                            .method(exact, name)
                            .expect("a method of a class that descends from the class of the call")
                            .func(),
                        Key::Str => self.exact_str(exact)?,
                        Key::Repr => self.exact_repr(exact)?,
                        Key::Truth => self.exact_truth(exact)?,
                    };
                    Ok((exact, body))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let sig = self.funcs.borrow().get(func).clone();
            let Some(&(_, first)) = arms.first() else {
                self.define(func, false, vec![Stmt::Unreachable]);
                continue;
            };
            let receiver = self.receiver(first);
            if arms.iter().all(|(_, body)| *body == first) && self.descends(class, receiver) {
                self.alias(func, first);
            } else {
                let body = self.dispatch_body(class, arms, &sig.params, sig.ret);
                self.define(func, false, body);
            }
        }
        Ok(self.classes.borrow().resolved > start)
    }

    /// The rep of the first parameter of `func`, a method.
    pub(super) fn self_rep(&self, func: FuncId) -> Rep {
        *self
            .funcs
            .borrow()
            .get(func)
            .params
            .first()
            .expect("a method has self")
    }

    /// The class of the first parameter of `func`, a method.
    pub(super) fn receiver(&self, func: FuncId) -> ClassId {
        self.self_rep(func).class().expect("self is an object")
    }

    /// The code of a function with `params`, the first an object of
    /// `class`, that calls the function beside the exact class of the
    /// object in `arms`, which has a class for each class that is `class`
    /// or descends from it. Each test names an exact type. The function
    /// that serves `class` takes any object without a cast, and serves the
    /// classes that the tests leave out. Without one, the last function
    /// serves the last class without a test.
    fn dispatch_body(
        &self,
        class: ClassId,
        mut arms: Vec<(ClassId, FuncId)>,
        params: &[Rep],
        ret: Rep,
    ) -> Vec<Stmt> {
        let call = |func: FuncId, object: Expr| {
            let args = std::iter::once(widen(object, self.self_rep(func)))
                .chain(
                    params
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(i, rep)| param(i, *rep)),
                )
                .collect();
            let value = generated_call(func, args, ret, Site::NONE);
            if ret == Rep::None {
                vec![Stmt::Expr(value), Stmt::Return(None)]
            } else {
                vec![Stmt::Return(Some(value))]
            }
        };
        let serves_all = |func: FuncId| self.descends(class, self.receiver(func));
        let shared = arms
            .iter()
            .map(|(_, func)| *func)
            .find(|func| serves_all(*func));
        if let Some(shared) = shared {
            arms.retain(|(_, func)| *func != shared);
        }
        let last = match shared {
            Some(func) => call(func, own(class)),
            None => {
                let (exact, func) = arms.pop().expect("an arm for the last class");
                call(func, as_exact(exact, class))
            }
        };
        let mut body: Vec<Stmt> = arms
            .into_iter()
            .map(|(exact, func)| Stmt::If {
                cond: Expr::IsExact {
                    value: Box::new(own(class)),
                    class: exact,
                },
                then: call(func, as_exact(exact, class)),
                else_: Vec::new(),
            })
            .collect();
        body.extend(last);
        body
    }

    /// The function that gives the `str` of an object of exactly `class`,
    /// which a member of an `Enum` writes as `Cor.VERDE`. The `str` of an
    /// object of a dataclass is its `repr`, and so is the one of an object
    /// of a class without `__str__`. The class `Enum` has its own.
    fn exact_str(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.conversion(class, "__str__")? {
            return Ok(func);
        }
        let (def, kind, own_str) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (declared.def, declared.kind, declared.own_str)
        };
        if kind != Kind::Enum {
            return self.exact_repr(class);
        }
        if let Some(func) = own_str {
            return Ok(func);
        }
        let body = vec![Stmt::Return(Some(Expr::Concat {
            left: Box::new(Expr::Str(format!("{}.", def.name))),
            right: Box::new(self.own_field(class, 0)),
            site: Site::NONE,
        }))];
        let func = self.generated(vec![Rep::Object(class)], Rep::Str, body);
        self.classes.borrow_mut().get_mut(class).own_str = Some(func);
        Ok(func)
    }

    /// The function of the method `name` of `class`, `__str__` or
    /// `__repr__`, and `None` when neither the class nor a class that it
    /// descends from defines it. A dataclass takes no `__repr__` from its
    /// base.
    fn conversion(&self, class: ClassId, name: &str) -> Result<Option<FuncId>, Error> {
        // The decorator of each dataclass writes its own `__repr__`.
        let method = if name == "__repr__" && self.is_dataclass(class) {
            self.classes.borrow().own_method(class, name)
        } else {
            self.method(class, name)
        };
        let Some(method) = method else {
            return Ok(None);
        };
        let func = method.func();
        let sig = self.funcs.borrow().get(func).clone();
        if sig.params.len() != 1 || sig.ret != Rep::Str {
            return Err(unsupported(
                method.def,
                "a conversion with these parameters",
            ));
        }
        Ok(Some(func))
    }

    /// The function that gives the `repr` of an object of exactly `class`,
    /// which a member of an `Enum` writes as `<Cor.VERDE: 1>`, with the
    /// `repr` of its value.
    fn exact_repr(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.conversion(class, "__repr__")? {
            return Ok(func);
        }
        let (def, kind, own_repr) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (declared.def, declared.kind, declared.own_repr)
        };
        if let Some(func) = own_repr {
            return Ok(func);
        }
        if kind.is_dataclass() {
            return self.dataclass_repr(class);
        }
        let name = def.name.to_string();
        if kind == Kind::Plain {
            // CPython writes the address of the object too, which changes
            // from one run to the next.
            let body = vec![Stmt::Return(Some(Expr::Str(format!(
                "<__main__.{name} object>"
            ))))];
            let func = self.generated(vec![Rep::Object(class)], Rep::Str, body);
            self.classes.borrow_mut().get_mut(class).own_repr = Some(func);
            return Ok(func);
        }
        let value = self.repr_at(self.own_field(class, 1), Site::NONE)?;
        let body = vec![Stmt::Return(Some(Expr::Join {
            parts: vec![
                Expr::Str(format!("<{name}.")),
                self.own_field(class, 0),
                Expr::Str(": ".to_owned()),
                value,
                Expr::Str(">".to_owned()),
            ],
            site: Site::NONE,
        }))];
        let func = self.generated(vec![Rep::Object(class)], Rep::Str, body);
        self.classes.borrow_mut().get_mut(class).own_repr = Some(func);
        Ok(func)
    }

    /// The function that gives the `repr` of an object of `class`, a
    /// dataclass, which writes `Tempo(horas=1, minutos=0)` with the `repr`
    /// of each field. As in CPython, the function `wrapper` of `reprlib`
    /// calls the function `__repr__` that the module `dataclasses` writes,
    /// and each one has a frame in a traceback.
    fn dataclass_repr(&self, class: ClassId) -> Result<FuncId, Error> {
        let (def, fields) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (declared.def, declared.fields.clone())
        };
        let params = vec![Rep::Object(class)];
        let wrapper = self.reserve(params.clone(), Rep::Str);
        let repr = self.reserve(params, Rep::Str);
        // A field may hold an object of the class itself.
        self.classes.borrow_mut().get_mut(class).own_repr = Some(wrapper);
        let (wrapper_site, repr_site) = self.repr_sites(fields.len());
        // The `wrapper` of `reprlib.recursive_repr` gives `...` for an
        // object whose `repr` runs.
        let call = generated_call(repr, vec![own(class)], Rep::Str, wrapper_site);
        self.define(
            wrapper,
            true,
            vec![Stmt::Return(Some(Expr::IfElse {
                cond: Box::new(Expr::InRepr(Box::new(own(class)))),
                then: Box::new(Expr::Str("...".to_owned())),
                else_: Box::new(Expr::DuringRepr {
                    object: Box::new(own(class)),
                    value: Box::new(call),
                }),
            }))],
        );
        let mut parts = vec![Expr::Str(format!("{}(", def.name))];
        for (i, (name, _)) in fields.iter().enumerate() {
            if i > 0 {
                parts.push(Expr::Str(", ".to_owned()));
            }
            parts.push(Expr::Str(format!("{name}=")));
            let field = u32::try_from(i).expect("fewer than 2^32 fields");
            parts.push(self.repr_at(self.own_field(class, field), repr_site)?);
        }
        parts.push(Expr::Str(")".to_owned()));
        self.define(
            repr,
            true,
            vec![Stmt::Return(Some(Expr::Join {
                parts,
                site: repr_site,
            }))],
        );
        Ok(wrapper)
    }

    /// The site of the call in the function `wrapper` of `reprlib`, and the
    /// site of the code of the `__repr__` of a dataclass with `fields`
    /// fields. The module `dataclasses` writes the `__init__` and then the
    /// `__repr__` in the same text, which puts the code of the `__repr__`
    /// on the line 5 plus the number of lines of the code of `__init__`.
    fn repr_sites(&self, fields: usize) -> (Site, Site) {
        let wrapper = self.add_site(
            format!("{REPRLIB}\nwrapper"),
            21,
            format!(
                "  File \"{REPRLIB}\", line 21, in wrapper\n    result = user_function(self)\n"
            ),
        );
        let line = 5 + fields.max(1);
        let repr = self.dataclass_site("__repr__", line);
        (wrapper, repr)
    }

    /// The site at `line` of the function `name` that the module
    /// `dataclasses` writes, whose text has the file `<string>`.
    fn dataclass_site(&self, name: &str, line: usize) -> Site {
        self.add_site(
            format!("<string>\n{name}"),
            line,
            format!("  File \"<string>\", line {line}, in {name}\n"),
        )
    }

    /// The object of `class` that `call` makes. The checker rejects a
    /// missing argument, an extra one and one that two names give, and an
    /// argument of a class without `__init__`.
    pub(super) fn construct(
        &self,
        call: &ast::ExprCall,
        name: &ast::ExprName,
        class: ClassId,
    ) -> Result<Expr, Error> {
        let kind = self.classes.borrow().get(class).kind;
        if kind != Kind::Enum {
            self.classes.borrow_mut().instantiated.insert(class);
        }
        let (init, bindings) = match kind {
            Kind::Enum => return Err(unsupported(call, "a call of this class")),
            Kind::Plain => {
                let Some((def, init)) = self.plain_init(class) else {
                    return self.after(Binding::Class(class), name, Expr::Empty(class));
                };
                (init, self.param_bindings(call, def)?)
            }
            Kind::Dataclass { .. } => (self.class_init(class), self.field_bindings(call, class)?),
        };
        let sig = self.funcs.borrow().get(init).clone();
        let (args, order) = self.arguments(call, &sig, Some(&bindings))?;
        let value = Expr::Call {
            func: init,
            args,
            order,
            rep: Rep::Object(class),
            site: self.call_site(init, call),
            check: None,
        };
        self.after(Binding::Class(class), name, value)
    }

    /// The call of `method`, a method of `class`, with the object that
    /// `receiver` gives as `self`. The object runs before the arguments,
    /// and `None` raises `AttributeError` there.
    pub(super) fn call_method(
        &self,
        call: &ast::ExprCall,
        attr: &ast::ExprAttribute,
        receiver: Expr,
        method: Method<'a>,
    ) -> Result<Expr, Error> {
        let text = format!("'NoneType' object has no attribute '{}'", attr.attr);
        let class = receiver.rep().class().expect("an object");
        if method.def.name.as_str() == "__init__" {
            self.check_init_overrides(call, class, method)?;
        }
        let func = self.method_slot(class, method);
        self.call_with(call, self.not_none(receiver, text, attr), method, func)
    }

    /// Checks that each `__init__` of a class that descends from `class`
    /// may take the place of `init` in the call `call`. A class may change
    /// the parameters of its `__init__`, which [`Self::declare_methods`]
    /// lets through, but a call of `__init__` through an object of `class`
    /// binds the arguments by the parameters of `init`.
    fn check_init_overrides(
        &self,
        call: &ast::ExprCall,
        class: ClassId,
        init: Method<'a>,
    ) -> Result<(), Error> {
        for descendant in self.descendants(class) {
            if let Some(own) = self.method(descendant, "__init__")
                && !std::ptr::eq(own.def, init.def)
            {
                self.check_override(own, init).map_err(|_| {
                    unsupported(
                        call,
                        "a call of __init__ that a subclass overrides otherwise",
                    )
                })?;
            }
        }
        Ok(())
    }

    /// The function that a call of `method` through an object of `class`
    /// calls, which finds the method of the exact class of the object.
    pub(super) fn method_slot(&self, class: ClassId, method: Method<'a>) -> FuncId {
        let body = self.funcs.borrow().get(method.func()).clone();
        let params = std::iter::once(Rep::Object(class))
            .chain(body.params.iter().skip(1).copied())
            .collect();
        let key = Key::Method(method.def.name.to_string());
        self.slot(class, key, params, body.ret, body.defaults.clone())
    }

    /// The call `call` of `func`, a function with the parameters of
    /// `method`, with `receiver`, an object, as `self`.
    fn call_with(
        &self,
        call: &ast::ExprCall,
        receiver: Expr,
        method: Method<'a>,
        func: FuncId,
    ) -> Result<Expr, Error> {
        let bindings = self.param_bindings(call, method.def)?;
        self.call_self(call, receiver, func, &bindings)
    }

    /// The call of `func`, a method, with `receiver` as `self` and the
    /// arguments of `call` after it, which `bindings` binds.
    fn call_self(
        &self,
        call: &ast::ExprCall,
        receiver: Expr,
        func: FuncId,
        bindings: &[ArgBinding],
    ) -> Result<Expr, Error> {
        let sig = self.funcs.borrow().get(func).clone();
        let mut rest = sig.clone();
        rest.params.remove(0);
        rest.defaults.remove(0);
        let (args, order) = self.arguments(call, &rest, Some(bindings))?;
        let receiver = widen(receiver, self.self_rep(func));
        let order = order.map(|order| {
            std::iter::once(0)
                .chain(order.into_iter().map(|arg| arg + 1))
                .collect()
        });
        Ok(Expr::Call {
            func,
            args: std::iter::once(receiver).chain(args).collect(),
            order,
            rep: sig.ret,
            site: self.call_site(func, call),
            check: None,
        })
    }

    /// The binding of each argument of `call` to a parameter of `def`, a
    /// method, after `self`.
    fn param_bindings(
        &self,
        call: &ast::ExprCall,
        def: &ast::StmtFunctionDef,
    ) -> Result<Vec<ArgBinding>, Error> {
        let names: Vec<&str> = params_of(def)
            .skip(1)
            .map(|param| param.name().as_str())
            .collect();
        self.bindings_by_name(call, &names)
    }

    /// The call `call` of the method of `attr`, whose object is `sup`,
    /// which is `super()`. The method comes from the base of the class of
    /// the method that the call is in, and the call goes to it directly,
    /// with the object of `self`. The `__init__` of a dataclass assigns the
    /// fields of the base, and the `__init__` of `object` does nothing.
    pub(super) fn super_call(
        &self,
        call: &ast::ExprCall,
        attr: &ast::ExprAttribute,
        sup: &ast::ExprCall,
    ) -> Result<Expr, Error> {
        if !sup.arguments.is_empty() {
            return Err(unsupported(sup, "super with arguments"));
        }
        let (class, _) = self
            .enclosing_method(call)
            .ok_or_else(|| unsupported(sup, "super() outside a method"))?;
        let base = self.classes.borrow().get(class).base;
        // `super()` reads the first variable of the frame, which no
        // method assigns, as `check_self` tells.
        let object = param(0, Rep::Object(class));
        match base.and_then(|base| self.method(base, attr.attr.as_str())) {
            Some(method) => self.call_with(call, object, method, method.func()),
            None if attr.attr.as_str() == "__init__" => match base {
                Some(base) if self.is_dataclass(base) => self.dataclass_reinit(call, object, base),
                _ => Ok(Expr::None),
            },
            None => Err(unsupported(attr, "this attribute of super()")),
        }
    }

    /// The call `call` of the `__init__` of `class`, a dataclass, with
    /// `object` as `self`.
    fn dataclass_reinit(
        &self,
        call: &ast::ExprCall,
        object: Expr,
        class: ClassId,
    ) -> Result<Expr, Error> {
        // The struct of a frozen dataclass keeps its fields immutable.
        if self.classes.borrow().get(class).kind != (Kind::Dataclass { frozen: false }) {
            return Err(unsupported(call, "super().__init__ of a frozen dataclass"));
        }
        let bindings = self.field_bindings(call, class)?;
        self.call_self(call, object, self.reinit(class), &bindings)
    }

    /// The function [`Declared::reinit`] of `class`.
    fn reinit(&self, class: ClassId) -> FuncId {
        if let Some(func) = self.classes.borrow().get(class).reinit {
            return func;
        }
        let (reps, defaults): (Vec<Rep>, Vec<Option<VarId>>) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            let reps = declared.fields.iter().map(|(_, rep)| *rep);
            let defaults = declared.defaults.iter().copied();
            std::iter::once((Rep::Object(class), None))
                .chain(reps.zip(defaults))
                .unzip()
        };
        let mut body: Vec<Stmt> = reps
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, rep)| Stmt::SetField {
                object: param(0, Rep::Object(class)),
                class,
                field: u32::try_from(i - 1).expect("a field of a struct"),
                value: param(i, *rep),
            })
            .collect();
        body.push(Stmt::Return(Some(Expr::None)));
        // The function assigns the fields and raises nothing.
        let func = self.reserve_with(reps, Rep::None, defaults, false);
        self.define(func, false, body);
        self.classes.borrow_mut().get_mut(class).reinit = Some(func);
        func
    }

    /// The class and the `def` of the method of the program whose code
    /// `node` is in, when the lowering is in the code of that method.
    fn enclosing_method(&self, node: &impl Ranged) -> Option<(ClassId, &'a ast::StmtFunctionDef)> {
        if self.in_prelude() {
            return None;
        }
        let classes = self.classes.borrow();
        let (class, method) = classes.list.iter().enumerate().find_map(|(i, declared)| {
            declared
                .methods
                .iter()
                .find(|method| method.def.range().contains_range(node.range()))
                .map(|method| (ClassId::new(i), method.def))
        })?;
        let scope = self.scope.borrow();
        let scope = scope.as_ref()?;
        // The code of an example of a docstring is in the range of the
        // `def` too.
        (scope.doctest.is_none() && scope.name == method.name.as_str()).then_some((class, method))
    }

    /// The bindings of the checker for the arguments of `call`, a call of
    /// the `__init__` of `class`, a dataclass, whose parameters are the
    /// fields.
    fn field_bindings(
        &self,
        call: &ast::ExprCall,
        class: ClassId,
    ) -> Result<Vec<ArgBinding>, Error> {
        let names: Vec<String> = self
            .classes
            .borrow()
            .get(class)
            .fields
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        self.bindings_by_name(call, &names)
    }

    /// The bindings of the checker for the arguments of `call`, with the
    /// index of each parameter in `names`. The index of the checker may
    /// count `self`, and the name finds the parameter.
    fn bindings_by_name(
        &self,
        call: &ast::ExprCall,
        names: &[&str],
    ) -> Result<Vec<ArgBinding>, Error> {
        Ok(call_bindings(self.model(), call)
            .ok_or_else(|| unsupported(call, "a call with these arguments"))?
            .into_iter()
            .map(|binding| ArgBinding {
                parameter: binding
                    .parameter_name
                    .as_deref()
                    .and_then(|param| names.iter().position(|name| *name == param)),
                parameter_name: binding.parameter_name,
                parameter_shape: None,
            })
            .collect())
    }

    /// The `def` of the `__init__` of `class`, a class of [`Kind::Plain`],
    /// with the function that makes an empty object, calls the `__init__`
    /// with it and the arguments, and returns it. The function has no
    /// frame, and `None` stands for a class without `__init__`.
    fn plain_init(&self, class: ClassId) -> Option<(&'a ast::StmtFunctionDef, FuncId)> {
        let init = self.method(class, "__init__")?;
        let (def, method) = (init.def, init.func());
        if let Some(func) = self.classes.borrow().get(class).init {
            return Some((def, func));
        }
        let sig = self.funcs.borrow().get(method).clone();
        let params = sig.params.get(1..).unwrap_or_default().to_vec();
        let defaults = sig.defaults.get(1..).unwrap_or_default().to_vec();
        let object = Rep::Object(class);
        let func = self.reserve_with(params.clone(), object, defaults, true);
        let local = params.len();
        let self_rep = *sig.params.first().expect("a method has self");
        let args = std::iter::once(widen(param(local, object), self_rep))
            .chain(params.iter().enumerate().map(|(i, rep)| param(i, *rep)))
            .collect();
        let body = vec![
            Stmt::Assign {
                targets: vec![Place::Local(VarId::new(local))],
                value: Expr::Empty(class),
            },
            Stmt::Expr(generated_call(method, args, Rep::None, Site::NONE)),
            Stmt::Return(Some(param(local, object))),
        ];
        self.define_with(func, false, &[object], body);
        self.classes.borrow_mut().get_mut(class).init = Some(func);
        Some((def, func))
    }

    /// The function that makes an object of `class`, a dataclass, with the
    /// value of each field in its parameters.
    fn class_init(&self, class: ClassId) -> FuncId {
        if let Some(func) = self.classes.borrow().get(class).init {
            return func;
        }
        let reps: Vec<Rep> = self
            .classes
            .borrow()
            .get(class)
            .fields
            .iter()
            .map(|(_, rep)| *rep)
            .collect();
        let fields = reps
            .iter()
            .enumerate()
            .map(|(i, rep)| param(i, *rep))
            .collect();
        let body = vec![Stmt::Return(Some(Expr::New { class, fields }))];
        let defaults = self.classes.borrow().get(class).defaults.clone();
        // The function makes the object and raises nothing.
        let func = self.reserve_with(reps, Rep::Object(class), defaults, false);
        self.define(func, false, body);
        self.classes.borrow_mut().get_mut(class).init = Some(func);
        func
    }

    /// The function that gives the `str`, or the `repr` when `repr` is
    /// `true`, of an object of `class` or `None`. The function has no
    /// frame, and the conversion of the object has the frame of the call.
    pub(super) fn or_none_conversion(&self, class: ClassId, repr: bool) -> Result<FuncId, Error> {
        if let Some(func) = *self.classes.borrow_mut().get_mut(class).or_none(repr) {
            return Ok(func);
        }
        let convert = self.class_conversion(class, repr);
        let param = || param(0, Rep::ObjectOrNone(class));
        let body = vec![Stmt::Return(Some(Expr::IfElse {
            cond: Box::new(Expr::IsNone(Box::new(param()))),
            then: Box::new(Expr::Str("None".to_owned())),
            else_: Box::new(generated_call(
                convert,
                vec![Expr::Narrow {
                    value: Box::new(param()),
                    rep: Rep::Object(class),
                }],
                Rep::Str,
                Site::NONE,
            )),
        }))];
        let func = self.generated(vec![Rep::ObjectOrNone(class)], Rep::Str, body);
        *self.classes.borrow_mut().get_mut(class).or_none(repr) = Some(func);
        Ok(func)
    }

    /// The field at `field` of the object of `class` that the first
    /// parameter of a function of the compiler holds.
    fn own_field(&self, class: ClassId, field: u32) -> Expr {
        let rep = self
            .classes
            .borrow()
            .get(class)
            .fields
            .get(field as usize)
            .expect("a field of the class")
            .1;
        Expr::Field {
            object: Box::new(own(class)),
            class,
            field,
            rep,
        }
    }

    /// Returns `true` if the lowered operands `left` and `right`, each with
    /// its node, may be objects of the same dataclass, `false` otherwise.
    pub(super) fn share_dataclass(
        &self,
        left: &(Expr, &ast::Expr),
        right: &(Expr, &ast::Expr),
    ) -> bool {
        let right = self.dataclasses_of(right);
        self.dataclasses_of(left)
            .iter()
            .any(|class| right.contains(class))
    }

    /// The dataclasses whose objects the lowered `operand`, with its node,
    /// may be.
    fn dataclasses_of(&self, (lowered, node): &(Expr, &ast::Expr)) -> Vec<ClassId> {
        self.dataclasses_in(lowered.rep(), || expr_shape(self.model(), node))
    }

    /// The dataclasses whose objects a value of `rep` may be, where `shape`
    /// gives the type of a uniform value.
    pub(super) fn dataclasses_in(
        &self,
        rep: Rep,
        shape: impl FnOnce() -> Option<Shape>,
    ) -> Vec<ClassId> {
        match rep {
            Rep::Object(class) | Rep::ObjectOrNone(class) if self.is_dataclass(class) => {
                self.descendants(class)
            }
            Rep::Uniform => self.uniform_dataclasses(shape()),
            _ => Vec::new(),
        }
    }

    /// Returns `true` if `class` is the `object` of the builtins, `false`
    /// otherwise.
    pub(super) fn is_object(&self, class: &CheckerClass) -> bool {
        class.known == Some(KnownClass::Object)
    }

    pub(super) fn is_enum(&self, class: ClassId) -> bool {
        self.classes.borrow().get(class).kind == Kind::Enum
    }

    pub(super) fn is_dataclass(&self, class: ClassId) -> bool {
        self.classes.borrow().get(class).kind.is_dataclass()
    }

    /// The classes of the program, in the order of their ids.
    pub(super) fn all_classes(&self) -> impl Iterator<Item = ClassId> + use<> {
        (0..self.classes.borrow().list.len()).map(ClassId::new)
    }

    /// The class of each object that a value of `shape` may be.
    pub(super) fn classes_in(&self, shape: Option<Shape>) -> Vec<ClassId> {
        let mut found = Vec::new();
        self.add_classes_in(shape, &mut found);
        found
    }

    fn add_classes_in(&self, shape: Option<Shape>, found: &mut Vec<ClassId>) {
        match shape {
            Some(Shape::Instance(class, _)) if self.is_object(&class) => {
                found.extend(self.all_classes());
            }
            // A value of a class may be an object of a subclass.
            Some(Shape::Instance(class, _) | Shape::EnumMember(class, _)) => {
                let class = self.classes.borrow().ids.get(&class).copied();
                if let Some(class) = class {
                    found.extend(self.descendants(class));
                }
            }
            Some(Shape::Union(shapes)) => {
                for shape in shapes {
                    self.add_classes_in(Some(shape), found);
                }
            }
            Some(Shape::TypeVar(name)) => {
                let types = self.types.borrow();
                if let Some((_, arg)) = types.iter().find(|(var, _)| *var == name) {
                    found.extend(arg.classes.iter().copied());
                }
            }
            _ => {}
        }
    }

    /// The dataclasses that a uniform value of `shape` may be, in the order
    /// of their ids.
    pub(super) fn uniform_dataclasses(&self, shape: Option<Shape>) -> Vec<ClassId> {
        self.uniform_classes(shape, |class| self.is_dataclass(class))
    }

    /// The classes that a uniform value of `shape` may be and whose `==`
    /// is other than the identity, the dataclasses and the classes with
    /// `__eq__` or `__ne__`, in the order of their ids.
    pub(super) fn uniform_eq_classes(&self, shape: Option<Shape>) -> Vec<ClassId> {
        self.uniform_classes(shape, |class| {
            self.is_dataclass(class)
                || self.method(class, "__eq__").is_some()
                || self.method(class, "__ne__").is_some()
        })
    }

    /// The classes that a uniform value of `shape` may be and that `keep`
    /// takes, in the order of their ids.
    pub(super) fn uniform_classes(
        &self,
        shape: Option<Shape>,
        keep: impl Fn(ClassId) -> bool,
    ) -> Vec<ClassId> {
        let mut found = self.classes_in(shape);
        found.retain(|class| keep(*class));
        found.sort_by_key(|class| class.index());
        found.dedup();
        found
    }

    /// The comparison `c`, with `ops` and the lowered `operands`, where two
    /// operands may be objects of the same dataclass. The `__eq__` of the
    /// class compares them.
    pub(super) fn dataclass_compare(
        &self,
        c: &ast::ExprCompare,
        ops: Vec<CmpOp>,
        operands: Vec<(Expr, &ast::Expr)>,
    ) -> Result<Expr, Error> {
        let what = (
            "a chain of comparisons of dataclasses",
            "an order of objects",
        );
        self.equality(c, (ops, operands), what, |left, right| {
            let theirs = self.dataclasses_of(&right);
            let mut shared = self.dataclasses_of(&left);
            shared.retain(|class| theirs.contains(class));
            self.dataclass_eq(left.0, right.0, shared, self.site(c.range()))
        })
    }

    /// `left == right` at `site`, where both may be objects of the
    /// dataclasses `classes`. The `__eq__` of a class compares two objects
    /// of the class.
    pub(super) fn dataclass_eq(
        &self,
        left: Expr,
        right: Expr,
        classes: Vec<ClassId>,
        site: Site,
    ) -> Result<Expr, Error> {
        if let (Some(a), Some(b)) = (left.rep().class(), right.rep().class())
            && a == b
        {
            // An object may be null, which `__eq__` does not take.
            let rep = Rep::ObjectOrNone(a);
            let coerced = |value| self.coerce(value, rep).expect("an object or None");
            return Ok(generated_call(
                self.eq_or_none(a)?,
                vec![coerced(left), coerced(right)],
                Rep::Bool,
                site,
            ));
        }
        let eqs = classes
            .into_iter()
            .map(|class| Ok((class, self.eq_or_none(class)?)))
            .collect::<Result<_, Error>>()?;
        let uniform = |value| {
            Box::new(
                self.coerce(value, Rep::Uniform)
                    .expect("a uniform value holds any value"),
            )
        };
        Ok(Expr::UniformEq {
            left: uniform(left),
            right: uniform(right),
            eqs,
            site,
        })
    }

    /// The function `__eq__` of `class`, a dataclass, as CPython 3.13
    /// writes it. Two objects are equal when they are the same object, or
    /// when each field of one is equal to the field of the other, in order.
    /// The code is on the line 10 plus the number of lines of the code of
    /// `__init__`, after the `__init__` and the `__repr__`.
    fn class_eq(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).eq {
            return Ok(func);
        }
        let (fields, nodes) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (declared.fields.clone(), declared.nodes.clone())
        };
        let object = Rep::Object(class);
        let func = self.reserve(vec![object, object], Rep::Bool);
        // A field may hold an object of the class itself.
        self.classes.borrow_mut().get_mut(class).eq = Some(func);
        let line = 10 + fields.len().max(1);
        let site = self.dataclass_site("__eq__", line);
        let other = || param(1, object);
        let same = fields
            .iter()
            .zip(nodes)
            .enumerate()
            .map(|(i, ((_, rep), (node, shape)))| {
                let field = u32::try_from(i).expect("fewer than 2^32 fields");
                let theirs = Expr::Field {
                    object: Box::new(other()),
                    class,
                    field,
                    rep: *rep,
                };
                self.field_eq(self.own_field(class, field), theirs, site, node, shape)
            })
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .reduce(|before, eq| Expr::logic(LogicOp::And, before, eq));
        let body = Expr::logic(
            LogicOp::Or,
            Expr::Is(Box::new(own(class)), Box::new(other())),
            same.unwrap_or(Expr::Bool(true)),
        );
        self.define(func, true, vec![Stmt::Return(Some(body))]);
        Ok(func)
    }

    /// Whether `ours` and `theirs`, the values of a field `node` of two
    /// objects of a dataclass, are equal, as a `bool`. The field has the
    /// type `shape`, and the code at `site` compares the values.
    fn field_eq(
        &self,
        ours: Expr,
        theirs: Expr,
        site: Site,
        node: &ast::StmtAnnAssign,
        shape: Shape,
    ) -> Result<Expr, Error> {
        let rep = ours.rep();
        if self.calls_eq(
            (rep, || Some(shape.clone())),
            (rep, || Some(shape.clone())),
            CmpOp::Eq,
        ) {
            return self.special_eq(
                CmpOp::Eq,
                (ours, Some(shape.clone())),
                (theirs, Some(shape)),
                site,
            );
        }
        if let Some(list) = rep.list() {
            let rep = Rep::ListOrNone(list);
            let coerced = |value| self.coerce(value, rep).expect("a list or None");
            return self.list_call_at(node, "eq", vec![coerced(ours), coerced(theirs)], site);
        }
        let classes = self.uniform_dataclasses(Some(shape));
        if !classes.is_empty() {
            return self.dataclass_eq(ours, theirs, classes, site);
        }
        // A union compares by the types of its values.
        if matches!(rep, Rep::StrOrNone | Rep::Uniform) {
            let uniform = |value| self.coerce(value, Rep::Uniform).expect("a uniform value");
            return Ok(Expr::Compare(
                Box::new(uniform(ours)),
                vec![(CmpOp::Eq, uniform(theirs))],
            ));
        }
        Ok(Expr::Compare(Box::new(ours), vec![(CmpOp::Eq, theirs)]))
    }

    /// The function that compares two values of `class`, a dataclass, that
    /// may be `None`. A value is equal to `None` when it is `None`, and the
    /// `__eq__` of the class of an object compares it with another object
    /// of the same class. Objects of two classes are equal when they are
    /// the same object, since the `__eq__` of each gives `NotImplemented`.
    /// The function has no frame.
    fn eq_or_none(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).eq_or_none {
            return Ok(func);
        }
        let param = |i| param(i, Rep::ObjectOrNone(class));
        let narrow = |i| Expr::Narrow {
            value: Box::new(param(i)),
            rep: Rep::Object(class),
        };
        let mut body = vec![Stmt::If {
            cond: Expr::logic(
                LogicOp::Or,
                Expr::IsNone(Box::new(param(0))),
                Expr::IsNone(Box::new(param(1))),
            ),
            then: vec![Stmt::Return(Some(Expr::Is(
                Box::new(param(0)),
                Box::new(param(1)),
            )))],
            else_: Vec::new(),
        }];
        let classes = self.descendants(class);
        if let [only] = *classes {
            let eq = self.class_eq(only)?;
            body.push(Stmt::Return(Some(generated_call(
                eq,
                vec![narrow(0), narrow(1)],
                Rep::Bool,
                Site::NONE,
            ))));
        } else {
            for exact in classes {
                let eq = self.class_eq(exact)?;
                let is_exact = |i| Expr::IsExact {
                    value: Box::new(narrow(i)),
                    class: exact,
                };
                let as_exact = |i| Expr::AsExact {
                    value: Box::new(narrow(i)),
                    class: exact,
                };
                body.push(Stmt::If {
                    cond: Expr::logic(LogicOp::And, is_exact(0), is_exact(1)),
                    then: vec![Stmt::Return(Some(generated_call(
                        eq,
                        vec![as_exact(0), as_exact(1)],
                        Rep::Bool,
                        Site::NONE,
                    )))],
                    else_: Vec::new(),
                });
            }
            body.push(Stmt::Return(Some(Expr::Is(
                Box::new(param(0)),
                Box::new(param(1)),
            ))));
        }
        let or_none = Rep::ObjectOrNone(class);
        let func = self.generated(vec![or_none, or_none], Rep::Bool, body);
        self.classes.borrow_mut().get_mut(class).eq_or_none = Some(func);
        Ok(func)
    }

    /// The class that `value`, the name `name`, reads, and `None` when it
    /// reads no class of the module or when the checker sees more than one
    /// class in it.
    pub(super) fn class_named(&self, value: &ast::Expr, name: &ast::ExprName) -> Option<ClassId> {
        let id = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(id))
        {
            return None;
        }
        let classes = self.classes.borrow();
        let named = classes.names.get(id).copied()?;
        match expr_shape(self.model(), value) {
            Some(Shape::Class(class)) => classes.ids.get(&class).copied(),
            // The checker sees each class of a name with two classes in a
            // function.
            _ if classes
                .list
                .iter()
                .filter(|c| c.def.name.as_str() == id)
                .count()
                > 1 =>
            {
                None
            }
            _ => Some(named),
        }
    }

    /// Refuses an annotation of `annotations` that reads a class before its
    /// `class` statement. CPython 3.13 evaluates an annotation without
    /// quotes when the statement that has it runs, and raises `NameError`
    /// there, while CPython 3.14 does not evaluate it.
    pub(super) fn check_annotations<'e>(
        &self,
        annotations: impl Iterator<Item = &'e ast::Expr>,
    ) -> Result<(), Error> {
        for name in annotations.flat_map(names_in) {
            let is_class = self.classes.borrow().names.contains_key(name.id.as_str());
            if is_class && self.global_check(name)?.is_some() {
                return Err(unsupported(
                    name,
                    "a class in an annotation before its `class` statement",
                ));
            }
        }
        Ok(())
    }

    /// The members of `def` in the order of the class, together with its
    /// methods, when `def` has the form of an `Enum` that the compiler
    /// takes. `mro` and `checked` are what the checker gives for the class.
    fn enum_members(
        &self,
        def: &'a ast::StmtClassDef,
        mro: &[CheckerClass],
        checked: Option<&[(String, Shape)]>,
    ) -> Result<(Vec<Member>, Vec<Method<'a>>), Error> {
        let is_enum = mro.len() == 3
            && mro
                .get(1)
                .is_some_and(|base| base.known == Some(KnownClass::Enum))
            && def.type_params.is_none()
            && def.arguments.as_deref().is_some_and(|arguments| {
                match (&*arguments.args, &*arguments.keywords) {
                    ([base], []) => self.names_known(def, base, KnownClass::Enum),
                    _ => false,
                }
            });
        if !is_enum {
            return Err(unsupported(
                def,
                "a class other than a dataclass or an Enum",
            ));
        }
        let mut members: Vec<Member> = Vec::new();
        let mut methods = Vec::new();
        // CPython gives `auto()` the greatest value before it, plus 1.
        let mut greatest: Option<i32> = None;
        for (i, stmt) in def.body.iter().enumerate() {
            let (target, value) = match stmt {
                // The class `Enum` defines the special methods.
                ast::Stmt::FunctionDef(method)
                    if is_special(&method.name) && !is_conversion(&method.name) =>
                {
                    return Err(unsupported(method, "a special method in an Enum"));
                }
                ast::Stmt::FunctionDef(method) => {
                    push_method(&mut methods, method)?;
                    continue;
                }
                _ if is_filler(i, stmt) => continue,
                ast::Stmt::Assign(ast::StmtAssign { targets, value, .. }) => match &**targets {
                    [ast::Expr::Name(target)] if !target.id.starts_with('_') => (target, value),
                    _ => return Err(unsupported(stmt, "this target in an Enum")),
                },
                _ => return Err(unsupported(stmt, "this statement in an Enum")),
            };
            let value = match &**value {
                ast::Expr::Call(call)
                    if call.arguments.is_empty()
                        && self.names_known(def, &call.func, KnownClass::Auto) =>
                {
                    let next = greatest.map_or(Some(1), |greatest| greatest.checked_add(1));
                    match next {
                        Some(next) => int_literal(call, Some(i64::from(next)))?,
                        None => {
                            return Err(unsupported(call, "an int that does not fit in 31 bits"));
                        }
                    }
                }
                // `self.expr` folds the minus of a literal.
                literal if is_int_literal(literal) || literal.is_string_literal_expr() => {
                    self.expr(value)?
                }
                _ => return Err(unsupported(&**value, "a member of an Enum with this value")),
            };
            let value = match value {
                Expr::Int(n) => Literal::Int(n),
                Expr::Str(text) => Literal::Str(text),
                _ => unreachable!("an int or a str literal"),
            };
            if let Literal::Int(n) = value {
                greatest = Some(greatest.map_or(n, |greatest| greatest.max(n)));
            }
            if let Some(first) = members.first()
                && first.value.rep() != value.rep()
            {
                return Err(unsupported(stmt, "an Enum with values of two types"));
            }
            if members.iter().any(|member| member.value == value) {
                return Err(unsupported(
                    stmt,
                    "two members of an Enum with the same value",
                ));
            }
            members.push(Member {
                name: target.id.to_string(),
                value,
            });
        }
        // The checker reasons about another Enum when it computes a value
        // otherwise, as for an `auto()` after a value that goes down.
        // The checker has no members for an `Enum` without them.
        let checked = checked.unwrap_or_default();
        let agrees = checked.len() == members.len()
            && checked.iter().zip(&members).all(|((name, shape), member)| {
                *name == member.name
                    && match (shape, &member.value) {
                        (Shape::IntLit(a), Literal::Int(b)) => *a == i64::from(*b),
                        (Shape::StrLit(a), Literal::Str(b)) => a == b,
                        _ => false,
                    }
            });
        if !agrees {
            return Err(unsupported(
                def,
                "an Enum whose values the checker computes otherwise",
            ));
        }
        Ok((members, methods))
    }

    /// Returns `true` if `expr` reads the class `known` of the standard
    /// library through a name that an import binds before `def`, as `Enum`
    /// or `enum.Enum` does, `false` otherwise.
    fn names_known(&self, def: &ast::StmtClassDef, expr: &ast::Expr, known: KnownClass) -> bool {
        matches!(
            self.imported_shape(def, expr),
            Some(Shape::Class(class)) if class.known == Some(known)
        )
    }

    /// The type of `expr`, when `expr` reads a name that an import binds
    /// before `def`, as `Enum` or `enum.Enum` does, and `None` otherwise.
    fn imported_shape(&self, def: &ast::StmtClassDef, expr: &ast::Expr) -> Option<Shape> {
        let name = match expr {
            ast::Expr::Name(name) => name,
            ast::Expr::Attribute(ast::ExprAttribute { value, .. }) => value.as_name_expr()?,
            _ => return None,
        };
        self.imported_before(name, def)
            .then(|| expr_shape(self.model(), expr))
            .flatten()
    }

    /// Checks that the one decorator of `def` is `dataclass`, which the
    /// compiler takes without arguments or with `frozen=True`, and returns
    /// `true` for `frozen=True`.
    fn dataclass_decorator(&self, def: &ast::StmtClassDef) -> Result<bool, Error> {
        let decorator = match &*def.decorator_list {
            [decorator] => &decorator.expression,
            _ => return Err(unsupported(def, "a class with these decorators")),
        };
        let (func, frozen) = match decorator {
            ast::Expr::Call(call) => match (&*call.arguments.args, &*call.arguments.keywords) {
                ([], []) => (&*call.func, false),
                ([], [keyword])
                    if keyword
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.as_str() == "frozen")
                        && matches!(
                            keyword.value,
                            ast::Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                        ) =>
                {
                    (&*call.func, true)
                }
                _ => return Err(unsupported(decorator, "a dataclass with these arguments")),
            },
            func => (func, false),
        };
        if !self.names_dataclass(def, func) {
            return Err(unsupported(decorator, "this decorator of a class"));
        }
        Ok(frozen)
    }

    /// Checks that `def`, a class with a decorator, with `base` or without
    /// a base, has the form of a dataclass that the compiler takes, and
    /// gives the name and the type of each field of its body, together
    /// with its methods. `info` is what the checker gives for the class.
    fn dataclass_form(
        &self,
        def: &'a ast::StmtClassDef,
        info: &ClassInfo,
        base: Option<ClassId>,
    ) -> Result<(Checked, Vec<Method<'a>>), Error> {
        if def.type_params.is_some()
            || (base.is_none()
                && (def
                    .arguments
                    .as_deref()
                    .is_some_and(|arguments| !arguments.is_empty())
                    || info.mro.len() != 2))
        {
            return Err(unsupported(def, "a dataclass with a base"));
        }
        let Some(checked) = &info.dataclass_fields else {
            return Err(unsupported(def, "this class"));
        };
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for (i, stmt) in def.body.iter().enumerate() {
            match stmt {
                // The decorator writes the special methods, and keeps one
                // that the class defines.
                ast::Stmt::FunctionDef(method)
                    if is_special(&method.name) && !is_conversion(&method.name) =>
                {
                    return Err(unsupported(method, "a special method in a dataclass"));
                }
                ast::Stmt::FunctionDef(method) => push_method(&mut methods, method)?,
                _ if is_filler(i, stmt) => {}
                ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                    target,
                    value,
                    simple: true,
                    ..
                }) => {
                    let ast::Expr::Name(name) = &**target else {
                        return Err(unsupported(stmt, "this statement in a dataclass"));
                    };
                    // The `__init__` of CPython takes `self` and the fields.
                    if name.id.as_str() == "self" {
                        return Err(unsupported(stmt, "a field named self"));
                    }
                    fields.push((name.id.as_str(), value.is_some()));
                }
                _ => return Err(unsupported(stmt, "this statement in a dataclass")),
            }
        }
        // The fields of the base come first, and a field that the class
        // declares again keeps its place with the default of the class.
        let mut merged: Vec<(&str, bool)> = Vec::new();
        for ancestor in self.ancestors(base) {
            for node in dataclass_fields(ancestor) {
                let name = node.target.as_name_expr().expect("a name").id.as_str();
                merge_field(&mut merged, name, node.value.is_some());
            }
        }
        for (name, default) in &fields {
            merge_field(&mut merged, name, *default);
        }
        // The checker leaves out of the fields an annotation such as
        // `ClassVar`, which the text alone does not tell apart.
        let agrees = checked.len() == merged.len()
            && checked.iter().zip(&merged).all(|(field, (name, default))| {
                field.name == *name && field.has_default == *default
            });
        if !agrees {
            return Err(unsupported(
                def,
                "a dataclass whose fields the checker computes otherwise",
            ));
        }
        let own = fields
            .iter()
            .map(|(name, _)| {
                let field = checked
                    .iter()
                    .find(|field| field.name == *name)
                    .expect("a field that the checker knows");
                ((*name).to_owned(), field.shape.clone())
            })
            .collect();
        Ok((own, methods))
    }

    /// The `class` statement of `base` and of each class that it descends
    /// from, from the first one, and none without a base.
    fn ancestors(&self, base: Option<ClassId>) -> Vec<&'a ast::StmtClassDef> {
        let classes = self.classes.borrow();
        let mut ancestors: Vec<_> = base
            .into_iter()
            .flat_map(|base| classes.chain(base))
            .map(|class| classes.get(class).def)
            .collect();
        ancestors.reverse();
        ancestors
    }

    /// The class of the program that `def` extends, when its one base is a
    /// name of such a class, and `None` otherwise. `info` is what the
    /// checker gives for the class, and `frozen` tells whether a dataclass
    /// is frozen, and `None` stands for a class without a decorator. A
    /// dataclass extends a dataclass, frozen as it is, and a class without
    /// a decorator extends one without a decorator.
    fn program_base(
        &self,
        def: &ast::StmtClassDef,
        info: &ClassInfo,
        frozen: Option<bool>,
    ) -> Result<Option<ClassId>, Error> {
        let Some(arguments) = def.arguments.as_deref() else {
            return Ok(None);
        };
        let ([ast::Expr::Name(name)], []) = (&*arguments.args, &*arguments.keywords) else {
            return Ok(None);
        };
        let classes = self.classes.borrow();
        let Some(&base) = info.mro.get(1).and_then(|class| classes.ids.get(class)) else {
            return Ok(None);
        };
        match (classes.get(base).kind, frozen) {
            (Kind::Plain, None) => Ok(Some(base)),
            (Kind::Dataclass { frozen: theirs }, Some(ours)) if theirs == ours => Ok(Some(base)),
            (Kind::Enum, _) => Err(unsupported(name, "an Enum as a base")),
            (Kind::Dataclass { .. }, None) => Err(unsupported(name, "a dataclass as a base")),
            (Kind::Plain, Some(_)) => Err(unsupported(
                name,
                "a class without a decorator as the base of a dataclass",
            )),
            // The checker rejects such a base, which raises `TypeError`.
            (Kind::Dataclass { .. }, Some(_)) => Err(unsupported(
                name,
                "a dataclass with a base that is frozen otherwise",
            )),
        }
    }

    /// The name and the type of each field of `def`, a class without a
    /// decorator, with `base` or without a base, together with its methods.
    /// `info` is what the checker gives for the class. Each field has an
    /// annotation in the body of the class or of a class that it descends
    /// from, and [`check_init`] proves that the `__init__` assigns it. The
    /// third value tells whether the `__init__` lets `self` escape.
    fn plain_form(
        &self,
        def: &'a ast::StmtClassDef,
        info: &ClassInfo,
        base: Option<ClassId>,
    ) -> Result<(Checked, Vec<Method<'a>>, bool), Error> {
        if def.type_params.is_some() && !self.in_prelude() {
            return Err(unsupported(def, "a generic class"));
        }
        let mut checked = Vec::new();
        let mut methods: Vec<Method> = Vec::new();
        for (i, stmt) in def.body.iter().enumerate() {
            match stmt {
                _ if is_filler(i, stmt) => {}
                ast::Stmt::AnnAssign(
                    field @ ast::StmtAnnAssign {
                        target,
                        value: None,
                        simple: true,
                        ..
                    },
                ) if target.is_name_expr() => {
                    let name = target.as_name_expr().expect("a name").id.to_string();
                    let shape = declared_shape(self.model(), field)
                        .ok_or_else(|| unsupported(stmt, "a field of this type"))?;
                    checked.push((name, shape));
                }
                // The compiler calls no special method other than `__init__`
                // for an operator or a builtin yet.
                ast::Stmt::FunctionDef(method)
                    if is_special(&method.name)
                        && method.name.as_str() != "__init__"
                        && !COMPILED_SPECIAL.contains(&method.name.as_str())
                        && !is_conversion(&method.name) =>
                {
                    return Err(unsupported(method, "this special method"));
                }
                ast::Stmt::FunctionDef(method) => push_method(&mut methods, method)?,
                _ => return Err(unsupported(stmt, "this statement in a class")),
            }
        }
        // The fields of the classes that the class descends from come
        // first, each with the node of its annotation.
        let mut fields: Vec<(&str, TextRange)> = Vec::new();
        for ancestor in self.ancestors(base) {
            for node in dataclass_fields(ancestor) {
                let name = node.target.as_name_expr().expect("a name").id.as_str();
                if !fields.iter().any(|(field, _)| *field == name) {
                    fields.push((name, node.range()));
                }
            }
        }
        let inherited = fields.len();
        for (node, (name, _)) in dataclass_fields(def).zip(&checked) {
            if !fields.iter().any(|(field, _)| *field == name) {
                fields.push((name.as_str(), node.range()));
            }
        }
        if let Some(attribute) = info
            .instance_attributes
            .iter()
            .find(|attribute| !fields.iter().any(|(name, _)| *name == attribute.name))
        {
            return Err(unsupported(
                def,
                &format!(
                    "the attribute {} without an annotation in the class",
                    attribute.name
                ),
            ));
        }
        let init = methods
            .iter()
            .find(|method| method.def.name.as_str() == "__init__");
        let base_escapes = base.is_some_and(|base| self.classes.borrow().get(base).init_escapes);
        // The `__init__` of `object` assigns nothing and lets nothing
        // escape.
        let base = Base {
            fields: fields.get(..inherited).unwrap_or_default(),
            escapes: base_escapes,
        };
        // A class without `__init__` takes the one of its base, which
        // assigns the fields of the base alone.
        let escapes = match (init, fields.get(inherited)) {
            (Some(init), _) => check_init(init.def, &fields, base)?,
            (None, Some((_, range))) => {
                return Err(unsupported(range, "a field that __init__ may not assign"));
            }
            (None, None) => base_escapes,
        };
        Ok((checked, methods, escapes))
    }

    /// Returns `true` if `expr` reads the function `dataclass` of the
    /// standard library through a name that an import binds before `def`,
    /// as `dataclass` or `dataclasses.dataclass` does, `false` otherwise.
    fn names_dataclass(&self, def: &ast::StmtClassDef, expr: &ast::Expr) -> bool {
        matches!(
            self.imported_shape(def, expr),
            Some(Shape::Function(func))
                if func.name == "dataclass"
                    && is_module(self.model().db(), func.file, KnownModule::Dataclasses)
        )
    }
}

impl Lowering<'_, '_> {
    /// Returns `true` if an import in the code of the module, out of any
    /// `if` or `while`, binds `name` before `def`, `false` otherwise. The
    /// `class` statement reads the name when it runs, and the read raises
    /// `NameError` before the import. No other statement binds a name of
    /// an import.
    fn imported_before(&self, name: &ast::ExprName, def: &ast::StmtClassDef) -> bool {
        self.module_before(def.start())
            .any(|stmt| imports(stmt, name.id.as_str()))
    }

    /// Returns `true` if a statement of the code of the module, out of any
    /// `if` or `while`, binds `name` before `start`, `false` otherwise. The
    /// module runs each such statement before the code at `start`, and no
    /// statement takes the value of a name away.
    pub(super) fn bound_before(&self, name: &str, start: TextSize) -> bool {
        let target =
            |target: &ast::Expr| matches!(target, ast::Expr::Name(n) if n.id.as_str() == name);
        self.module_before(start).any(|stmt| match stmt {
            ast::Stmt::FunctionDef(def) => def.name.as_str() == name,
            ast::Stmt::ClassDef(def) => def.name.as_str() == name,
            ast::Stmt::Assign(a) => a.targets.iter().any(target),
            ast::Stmt::AnnAssign(a) => a.value.is_some() && target(&a.target),
            ast::Stmt::AugAssign(a) => target(&a.target),
            stmt => imports(stmt, name),
        })
    }

    /// The statements of the code of the module that end before `start`.
    fn module_before(&self, start: TextSize) -> impl Iterator<Item = &ast::Stmt> {
        self.unit
            .get()
            .suite
            .iter()
            .take_while(move |stmt| stmt.end() <= start)
    }
}

/// Returns `true` if `stmt` is an import that binds `name`, `false`
/// otherwise.
fn imports(stmt: &ast::Stmt, name: &str) -> bool {
    match stmt {
        ast::Stmt::Import(i) => i
            .names
            .iter()
            .any(|alias| imported_name(alias, false) == name),
        ast::Stmt::ImportFrom(i) => i
            .names
            .iter()
            .any(|alias| imported_name(alias, true) == name),
        _ => false,
    }
}

/// Adds the field `name` to `fields`, the name of each field of a dataclass
/// with whether it has a default. A field that `fields` has keeps its
/// place and takes the default.
fn merge_field<'n>(fields: &mut Vec<(&'n str, bool)>, name: &'n str, default: bool) {
    match fields.iter_mut().find(|(field, _)| *field == name) {
        Some(field) => field.1 = default,
        None => fields.push((name, default)),
    }
}

/// Adds `method` to `methods`, the methods of a class before it.
fn push_method<'a>(
    methods: &mut Vec<Method<'a>>,
    method: &'a ast::StmtFunctionDef,
) -> Result<(), Error> {
    if methods.iter().any(|known| known.def.name == method.name) {
        return Err(unsupported(method, "two methods of the same name"));
    }
    methods.push(Method {
        def: method,
        func: None,
    });
    Ok(())
}

/// The special methods other than `__init__`, `__str__` and `__repr__`
/// that an operator or a builtin calls.
pub(super) const COMPILED_SPECIAL: &[&str] = &[
    "__eq__",
    "__ne__",
    "__lt__",
    "__le__",
    "__gt__",
    "__ge__",
    "__add__",
    "__radd__",
    "__sub__",
    "__rsub__",
    "__mul__",
    "__rmul__",
    "__truediv__",
    "__rtruediv__",
    "__floordiv__",
    "__rfloordiv__",
    "__mod__",
    "__rmod__",
    "__pow__",
    "__rpow__",
    "__neg__",
    "__len__",
    "__bool__",
    "__contains__",
    "__getitem__",
    "__setitem__",
    "__iadd__",
    "__isub__",
    "__imul__",
    "__itruediv__",
    "__ifloordiv__",
    "__imod__",
    "__ipow__",
];

/// Returns `true` if `name` is the name of a special method, as `__eq__`
/// is, `false` otherwise.
fn is_special(name: &str) -> bool {
    name.len() > 4 && name.starts_with("__") && name.ends_with("__")
}

/// Returns `true` if `name` is `__str__` or `__repr__`, the methods that
/// give the text of an object, `false` otherwise.
fn is_conversion(name: &str) -> bool {
    matches!(name, "__str__" | "__repr__")
}

/// The object in the first parameter of a function of the compiler for an
/// object of `class`.
fn own(class: ClassId) -> Expr {
    param(0, Rep::Object(class))
}

/// `object`, an object of a class, as a value of `rep`, an object of the
/// class or of a class that it descends from.
pub(super) fn widen(object: Expr, rep: Rep) -> Expr {
    if object.rep() == rep {
        return object;
    }
    Expr::Widen {
        value: Box::new(object),
        rep,
    }
}

/// The object in the first parameter of a function of the compiler for an
/// object of `class`, as an object of exactly `exact`.
fn as_exact(exact: ClassId, class: ClassId) -> Expr {
    Expr::AsExact {
        value: Box::new(own(class)),
        class: exact,
    }
}

/// The parameter at `i` of a function of the compiler, a value of `rep`.
pub(super) fn param(i: usize, rep: Rep) -> Expr {
    Expr::Load {
        place: Place::Local(VarId::new(i)),
        rep,
        check: None,
    }
}

/// The call of `func`, a function of the compiler, with `args`, which
/// gives a value of `rep` and raises at `site`.
pub(super) fn generated_call(func: FuncId, args: Vec<Expr>, rep: Rep, site: Site) -> Expr {
    Expr::Call {
        func,
        args,
        order: None,
        rep,
        site: Some(site),
        check: None,
    }
}

/// The path of the module `reprlib` of CPython 3.13 on Linux.
const REPRLIB: &str = "/usr/lib/python3.13/reprlib.py";

/// The node of each field of `def`, a dataclass of the accepted form, in
/// order.
fn dataclass_fields(def: &ast::StmtClassDef) -> impl Iterator<Item = &ast::StmtAnnAssign> {
    def.body.iter().filter_map(|stmt| match stmt {
        ast::Stmt::AnnAssign(field) => Some(field),
        _ => None,
    })
}

/// Returns `true` if `expr` is an `int` literal, or the minus of one,
/// `false` otherwise.
fn is_int_literal(expr: &ast::Expr) -> bool {
    let expr = match expr {
        ast::Expr::UnaryOp(ast::ExprUnaryOp {
            op: ast::UnaryOp::USub,
            operand,
            ..
        }) => operand,
        expr => expr,
    };
    matches!(
        expr,
        ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: ast::Number::Int(_),
            ..
        })
    )
}

/// Returns `true` if `stmt`, the statement `i` of the body of a class, is
/// `pass`, `...` or the docstring, which do nothing, `false` otherwise.
fn is_filler(i: usize, stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Pass(_) => true,
        ast::Stmt::Expr(e) => {
            e.value.is_ellipsis_literal_expr() || (i == 0 && e.value.is_string_literal_expr())
        }
        _ => false,
    }
}
