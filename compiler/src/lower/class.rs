//! The classes of the module. The compiler reads a fixed form of a
//! dataclass and of an `Enum` in the text and asks the checker for the
//! names and the types. The compiled program runs no code of the
//! decorator, of the base nor of the annotations.

use std::collections::HashMap;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_text_size::{Ranged, TextRange, TextSize};
use ty_module_resolver::KnownModule;
use ty_python_semantic::types::KnownClass;
use ty_python_semantic::types::lowering::{
    ArgBinding, ClassId as CheckerClass, ClassInfo, Shape, call_bindings, class_info, expr_shape,
};

use super::call::is_module;
use super::expr::int_literal;
use super::flow::imported_name;
use super::{HashSet, Lowering, Scope, Vars, unsupported};
use crate::Error;
use crate::cpython::Class;
use crate::cpython::dataclasses::{CLASS_NAMESPACE, mutable_default, mutable_default_frames};
use crate::ir::{
    Binding, ClassId, CmpOp, Expr, FuncId, LogicOp, Place, Rep, Site, Stmt, UserClass, VarId,
};

/// The classes of the module.
#[derive(Default)]
pub(super) struct Classes<'a> {
    /// The class that each name of the module binds.
    pub names: HashMap<String, ClassId>,
    /// The class of each class of the checker.
    pub ids: HashMap<CheckerClass, ClassId>,
    pub list: Vec<Declared<'a>>,
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

    /// The classes of the IR.
    pub fn into_ir(self) -> Vec<UserClass> {
        self.list
            .into_iter()
            .map(|class| UserClass {
                ordered: class.ordered,
                fields: class.fields.into_iter().map(|(_, rep)| rep).collect(),
                mutable: class.kind == Kind::Dataclass && !class.frozen,
                members: class
                    .members
                    .into_iter()
                    .map(|member| vec![Expr::Str(member.name), member.value])
                    .collect(),
                str: class.str,
                repr_guard: class.kind == Kind::Dataclass,
            })
            .collect()
    }
}

/// A class of the module, with what the lowering knows of it.
pub(super) struct Declared<'a> {
    pub def: &'a ast::StmtClassDef,
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
    pub kind: Kind,
    /// `true` for a dataclass with `frozen=True`, whose fields keep their
    /// first value.
    pub frozen: bool,
    /// The name and the rep of each field of an object of the class. A
    /// member of an `Enum` has `name` and `value`.
    pub fields: Vec<(String, Rep)>,
    pub members: Vec<Member>,
    /// The function that gives the `str` of an object, once a conversion
    /// needs it.
    pub str: Option<FuncId>,
    /// The function that gives the `repr` of an object, once a conversion
    /// needs it.
    pub repr: Option<FuncId>,
    /// The function that makes an object of a dataclass from a value for
    /// each field, once a call needs it.
    pub init: Option<FuncId>,
    /// The function `__eq__` of a dataclass, once a comparison needs it.
    pub eq: Option<FuncId>,
    /// The function that compares two values of a dataclass that may be
    /// `None`, once a comparison needs it.
    pub eq_or_none: Option<FuncId>,
    /// The functions that give the `str` and the `repr` of an object of the
    /// class or `None`, once a conversion needs them.
    pub or_none: [Option<FuncId>; 2],
    /// The variable of the module that holds the default of each field of
    /// a dataclass that has one. The `class` statement gives it its value.
    pub defaults: Vec<Option<VarId>>,
}

/// The forms of a class that the compiler takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Enum,
    Dataclass,
}

/// A member of an `Enum`, with its value.
pub(super) struct Member {
    pub name: String,
    pub value: Expr,
}

impl<'a> Lowering<'a, '_> {
    /// Declares the class of `def`, a `class` statement of the module. The
    /// fields of a dataclass wait for [`Self::declare_fields`], since they
    /// may hold an object of a class that comes later.
    pub(super) fn declare_class(&self, def: &'a ast::StmtClassDef) -> Result<(), Error> {
        let name = def.name.as_str();
        if self.imports.borrow().ids.contains_key(name) {
            return Err(unsupported(&def.name, "a name of an import and of a class"));
        }
        if self.classes.borrow().names.contains_key(name) {
            return Err(unsupported(&def.name, "two classes of the same name"));
        }
        // A decorator other than `dataclass` may make the class something
        // else, which the checker does not describe.
        let frozen = if def.decorator_list.is_empty() {
            false
        } else {
            self.dataclass_decorator(def)?
        };
        let info = class_info(self.model(), def).ok_or_else(|| unsupported(def, "this class"))?;
        let (kind, fields, members) = if def.decorator_list.is_empty() {
            let members = self.enum_members(def, &info.mro, info.enum_members.as_deref())?;
            let value = members
                .first()
                .map_or(Rep::Int, |member| member.value.rep());
            let fields = vec![("name".to_owned(), Rep::Str), ("value".to_owned(), value)];
            (Kind::Enum, fields, members)
        } else {
            self.dataclass_form(def, &info)?;
            (Kind::Dataclass, Vec::new(), Vec::new())
        };
        let mut classes = self.classes.borrow_mut();
        let id = ClassId::new(classes.list.len());
        classes.list.push(Declared {
            def,
            ordered: false,
            kind,
            frozen,
            fields,
            members,
            str: None,
            repr: None,
            init: None,
            eq: None,
            eq_or_none: None,
            or_none: [None; 2],
            defaults: Vec::new(),
        });
        classes.names.insert(name.to_owned(), id);
        classes.ids.insert(info.id, id);
        Ok(())
    }

    /// Gives each field of each dataclass of the module the rep of its
    /// values, once every class of the module has an id.
    pub(super) fn declare_fields(&self) -> Result<(), Error> {
        let count = self.classes.borrow().list.len();
        for class in (0..count).map(ClassId::new) {
            let (def, kind) = {
                let classes = self.classes.borrow();
                let declared = classes.get(class);
                (declared.def, declared.kind)
            };
            if kind != Kind::Dataclass {
                continue;
            }
            let info = class_info(self.model(), def).expect("a class that the checker knows");
            let fields = info
                .dataclass_fields
                .expect("a dataclass that the checker knows")
                .into_iter()
                .zip(dataclass_fields(def))
                .map(|(field, node)| {
                    let rep = self
                        .rep_of(Some(field.shape))
                        .filter(|rep| *rep != Rep::None)
                        .ok_or_else(|| unsupported(node, "a field of this type"))?;
                    let default = node
                        .value
                        .as_ref()
                        .map(|_| self.vars.borrow_mut().add_unnamed(rep));
                    Ok(((field.name, rep), default))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let (fields, defaults) = fields.into_iter().unzip();
            let mut classes = self.classes.borrow_mut();
            let declared = classes.get_mut(class);
            declared.fields = fields;
            declared.defaults = defaults;
        }
        Ok(())
    }

    /// The class of `def`, a `class` statement that the lowering finds in
    /// the code.
    pub(super) fn class_of(&self, def: &ast::StmtClassDef) -> Result<ClassId, Error> {
        if self.scope.borrow().is_some() {
            return Err(unsupported(def, "a class in a function"));
        }
        let classes = self.classes.borrow();
        classes
            .names
            .get(def.name.as_str())
            .copied()
            .filter(|id| std::ptr::eq(classes.get(*id).def, def))
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
        if kind != Kind::Dataclass {
            return Ok(Vec::new());
        }
        let mut namespace: Vec<String> = CLASS_NAMESPACE.map(str::to_owned).into();
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
        let body = self.class_defaults(def, &fields, &defaults);
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
            stmts.push(Stmt::Expr(Expr::Call {
                func,
                args: Vec::new(),
                order: None,
                rep: Rep::None,
                site: Some(self.site(range)),
                check: None,
            }));
        }
        stmts.extend(mutable);
        Ok(stmts)
    }

    /// The assignment of each default of `def`, a dataclass with `fields`
    /// and `defaults`, in the scope of its body, and the raise of the
    /// decorator for the first default that is an object of a dataclass.
    fn class_defaults(
        &self,
        def: &ast::StmtClassDef,
        fields: &[(String, Rep)],
        defaults: &[Option<VarId>],
    ) -> Result<(Vec<Stmt>, Option<Stmt>), Error> {
        let mut body = Vec::new();
        let mut mutable = None;
        for ((node, (name, rep)), var) in dataclass_fields(def).zip(fields).zip(defaults) {
            if let (Some(value), Some(var)) = (node.value.as_deref(), var) {
                body.push(self.class_default(def, value, name, *rep, *var, &mut mutable)?);
            }
            self.check_annotations(std::iter::once(&*node.annotation))?;
        }
        Ok((body, mutable))
    }

    /// The assignment of `value`, the default of the field `name` of `def`,
    /// to `var`, which holds values of `rep`. `mutable` gets the raise of
    /// the decorator when the default is the first that is an object of a
    /// dataclass.
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
        match lowered.rep() {
            Rep::Object(class) if mutable.is_none() && self.is_mutable(class) => {
                let object = self.classes.borrow().get(class).def.name.to_string();
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
            Rep::ObjectOrNone(_) | Rep::Uniform
                if self.may_be_mutable(expr_shape(self.model(), value)) =>
            {
                return Err(unsupported(
                    value,
                    "a default that may be an object of a dataclass",
                ));
            }
            _ => {}
        }
        let lowered = self
            .coerce(lowered, rep)
            .ok_or_else(|| unsupported(value, "a default of this type"))?;
        if let Some(scope) = self.scope.borrow_mut().as_mut() {
            scope.namespace.push(name.to_owned());
        }
        Ok(Stmt::Assign {
            targets: vec![Place::Global(var)],
            value: lowered,
        })
    }

    /// Returns `true` if a value of `shape` may be an object of a class
    /// that [`Self::is_mutable`] accepts, `false` otherwise.
    fn may_be_mutable(&self, shape: Option<Shape>) -> bool {
        let mut found = Vec::new();
        self.classes_in(shape, &mut found);
        found.into_iter().any(|class| self.is_mutable(class))
    }

    /// Returns `true` if `class` is a dataclass whose fields a program may
    /// assign, `false` otherwise. CPython calls a default of such a class
    /// mutable, because the `__eq__` of the class takes its hash away.
    fn is_mutable(&self, class: ClassId) -> bool {
        let classes = self.classes.borrow();
        let declared = classes.get(class);
        declared.kind == Kind::Dataclass && !declared.frozen
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
        Ok(match self.global_check(name)? {
            Some(check) => Expr::After {
                binding: Binding::Class(class),
                check,
                value: Box::new(member),
            },
            None => member,
        })
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
    fn not_none(&self, object: Expr, text: String, attr: &ast::ExprAttribute) -> Expr {
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
            Some(Rep::Object(class)) if self.is_mutable(class) => class,
            _ => return Err(unsupported(attr, "this target")),
        };
        let object = self.expr(&attr.value)?;
        let (field, rep) = self.field_of(attr, class)?;
        Ok((object, class, field, rep))
    }

    /// The index and the rep of the field of `class` that `attr` names.
    fn field_of(&self, attr: &ast::ExprAttribute, class: ClassId) -> Result<(u32, Rep), Error> {
        let classes = self.classes.borrow();
        let (field, (_, rep)) = classes
            .get(class)
            .fields
            .iter()
            .enumerate()
            .find(|(_, (name, _))| *name == attr.attr.as_str())
            .ok_or_else(|| unsupported(attr, "this attribute of an object"))?;
        Ok((u32::try_from(field).expect("fewer than 2^32 fields"), *rep))
    }

    /// The function that gives the `str` of an object of `class`, which a
    /// member of an `Enum` writes as `Cor.VERDE`.
    pub(super) fn class_str(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).str {
            return Ok(func);
        }
        let (def, kind) = {
            let classes = self.classes.borrow();
            (classes.get(class).def, classes.get(class).kind)
        };
        // The `str` of an object of a dataclass is its `repr`.
        if kind == Kind::Dataclass {
            let func = self.class_repr(class)?;
            self.classes.borrow_mut().get_mut(class).str = Some(func);
            return Ok(func);
        }
        let name = def.name.to_string();
        let body = vec![Stmt::Return(Some(Expr::Concat {
            left: Box::new(Expr::Str(format!("{name}."))),
            right: Box::new(self.own_field(class, 0)),
            site: Site::NONE,
        }))];
        let func = self.generated(vec![Rep::Object(class)], Rep::Str, body);
        self.classes.borrow_mut().get_mut(class).str = Some(func);
        Ok(func)
    }

    /// The function that gives the `repr` of an object of `class`, which a
    /// member of an `Enum` writes as `<Cor.VERDE: 1>`, with the `repr` of
    /// its value.
    pub(super) fn class_repr(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).repr {
            return Ok(func);
        }
        let (def, kind) = {
            let classes = self.classes.borrow();
            (classes.get(class).def, classes.get(class).kind)
        };
        if kind == Kind::Dataclass {
            return self.dataclass_repr(class);
        }
        let name = def.name.to_string();
        let value = self.own_field(class, 1);
        let value = match value.rep() {
            Rep::Str => self.prelude_call_at("str", "repr_of", vec![value], Site::NONE)?,
            _ => Expr::StrOf {
                operand: Box::new(value),
                site: Site::NONE,
            },
        };
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
        self.classes.borrow_mut().get_mut(class).repr = Some(func);
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
        self.classes.borrow_mut().get_mut(class).repr = Some(wrapper);
        let (wrapper_site, repr_site) = self.repr_sites(fields.len());
        // The `wrapper` of `reprlib.recursive_repr` gives `...` for an
        // object whose `repr` runs.
        let call = Expr::Call {
            func: repr,
            args: vec![own(class)],
            order: None,
            rep: Rep::Str,
            site: Some(wrapper_site),
            check: None,
        };
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
        for (i, ((name, rep), node)) in fields.iter().zip(dataclass_fields(def)).enumerate() {
            if i > 0 {
                parts.push(Expr::Str(", ".to_owned()));
            }
            parts.push(Expr::Str(format!("{name}=")));
            let field = u32::try_from(i).expect("fewer than 2^32 fields");
            parts.push(self.field_repr(class, field, *rep, repr_site, node)?);
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
        let repr = self.add_site(
            "<string>\n__repr__".to_owned(),
            line,
            format!("  File \"<string>\", line {line}, in __repr__\n"),
        );
        (wrapper, repr)
    }

    /// The `repr` of the field at `field` of the object of `class` in the
    /// first parameter, a value of `rep`, which the code at `site` gives.
    /// `node` is the field in the class.
    fn field_repr(
        &self,
        class: ClassId,
        field: u32,
        rep: Rep,
        site: Site,
        node: &ast::StmtAnnAssign,
    ) -> Result<Expr, Error> {
        let value = self.own_field(class, field);
        Ok(match rep {
            Rep::Int | Rep::Float | Rep::Bool => Expr::StrOf {
                operand: Box::new(value),
                site,
            },
            Rep::Str => self.prelude_call_at("str", "repr_of", vec![value], site)?,
            Rep::StrOrNone | Rep::Object(_) | Rep::ObjectOrNone(_) => {
                // A field of an object may be null.
                let some = match rep {
                    Rep::Object(_) => self.own_field(class, field),
                    Rep::ObjectOrNone(other) => Expr::Narrow {
                        value: Box::new(self.own_field(class, field)),
                        rep: Rep::Object(other),
                    },
                    _ => Expr::Narrow {
                        value: Box::new(self.own_field(class, field)),
                        rep: Rep::Str,
                    },
                };
                Expr::IfElse {
                    cond: Box::new(Expr::IsNone(Box::new(value))),
                    then: Box::new(Expr::Str("None".to_owned())),
                    else_: Box::new(self.field_repr_of(some, site)?),
                }
            }
            Rep::Uniform | Rep::None => {
                return Err(unsupported(node, "the repr of a field of a union"));
            }
        })
    }

    /// The `repr` of `value`, a `str` or an object, which the code at
    /// `site` gives.
    fn field_repr_of(&self, value: Expr, site: Site) -> Result<Expr, Error> {
        Ok(match value.rep() {
            Rep::Object(class) => Expr::Call {
                func: self.class_repr(class)?,
                args: vec![value],
                order: None,
                rep: Rep::Str,
                site: Some(site),
                check: None,
            },
            _ => self.prelude_call_at("str", "repr_of", vec![value], site)?,
        })
    }

    /// The object of `class`, a dataclass, that `call` makes. The checker
    /// rejects a missing argument, an extra one and one that two names give.
    pub(super) fn construct(
        &self,
        call: &ast::ExprCall,
        name: &ast::ExprName,
        class: ClassId,
    ) -> Result<Expr, Error> {
        if self.classes.borrow().get(class).kind != Kind::Dataclass {
            return Err(unsupported(call, "a call of this class"));
        }
        let init = self.class_init(class);
        let sig = self.funcs.borrow().get(init).clone();
        let names: Vec<String> = self
            .classes
            .borrow()
            .get(class)
            .fields
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        // The index of a parameter of the checker may count `self`, and the
        // name finds the field.
        let bindings: Vec<ArgBinding> = call_bindings(self.model(), call)
            .ok_or_else(|| unsupported(call, "a call with these arguments"))?
            .into_iter()
            .map(|binding| ArgBinding {
                parameter: binding
                    .parameter_name
                    .as_deref()
                    .and_then(|param| names.iter().position(|name| name == param)),
                parameter_name: binding.parameter_name,
                parameter_shape: None,
            })
            .collect();
        let (args, order) = self.arguments(call, &sig, Some(&bindings))?;
        let value = Expr::Call {
            func: init,
            args,
            order,
            rep: Rep::Object(class),
            site: self.call_site(init, call),
            check: None,
        };
        Ok(match self.global_check(name)? {
            Some(check) => Expr::After {
                binding: Binding::Class(class),
                check,
                value: Box::new(value),
            },
            None => value,
        })
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
            .map(|(i, rep)| Expr::Load {
                place: Place::Local(VarId::new(i)),
                rep: *rep,
                check: None,
            })
            .collect();
        let body = vec![Stmt::Return(Some(Expr::New { class, fields }))];
        let func = self.generated(reps, Rep::Object(class), body);
        let defaults = self.classes.borrow().get(class).defaults.clone();
        if let Some(sig) = self.funcs.borrow_mut().sigs.get_mut(func.index()) {
            sig.defaults = defaults;
        }
        // The function makes the object and raises nothing.
        if let Some(raises) = self.funcs.borrow_mut().raises.get_mut(func.index()) {
            *raises = false;
        }
        self.classes.borrow_mut().get_mut(class).init = Some(func);
        func
    }

    /// The function that gives the `str`, or the `repr` when `repr` is
    /// `true`, of an object of `class` or `None`. The function has no
    /// frame, and the conversion of the object has the frame of the call.
    pub(super) fn or_none_conversion(&self, class: ClassId, repr: bool) -> Result<FuncId, Error> {
        let index = usize::from(repr);
        let cached = self
            .classes
            .borrow()
            .get(class)
            .or_none
            .get(index)
            .copied()
            .flatten();
        if let Some(func) = cached {
            return Ok(func);
        }
        let convert = if repr {
            self.class_repr(class)?
        } else {
            self.class_str(class)?
        };
        let param = || Expr::Load {
            place: Place::Local(VarId::new(0)),
            rep: Rep::ObjectOrNone(class),
            check: None,
        };
        let body = vec![Stmt::Return(Some(Expr::IfElse {
            cond: Box::new(Expr::IsNone(Box::new(param()))),
            then: Box::new(Expr::Str("None".to_owned())),
            else_: Box::new(Expr::Call {
                func: convert,
                args: vec![Expr::Narrow {
                    value: Box::new(param()),
                    rep: Rep::Object(class),
                }],
                order: None,
                rep: Rep::Str,
                site: Some(Site::NONE),
                check: None,
            }),
        }))];
        let func = self.generated(vec![Rep::ObjectOrNone(class)], Rep::Str, body);
        if let Some(slot) = self
            .classes
            .borrow_mut()
            .get_mut(class)
            .or_none
            .get_mut(index)
        {
            *slot = Some(func);
        }
        Ok(func)
    }

    /// The call of `func`, a function of the compiler that converts the
    /// object `object` to a `str`, for the conversion at `range`.
    pub(super) fn class_call(&self, func: FuncId, object: Expr, range: TextRange) -> Expr {
        Expr::Call {
            func,
            args: vec![object],
            order: None,
            rep: Rep::Str,
            site: Some(self.site(range)),
            check: None,
        }
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
        let mut found = Vec::new();
        match lowered.rep() {
            Rep::Object(class) | Rep::ObjectOrNone(class) => found.push(class),
            Rep::Uniform => self.classes_in(expr_shape(self.model(), node), &mut found),
            _ => {}
        }
        let classes = self.classes.borrow();
        found.retain(|class| classes.get(*class).kind == Kind::Dataclass);
        found
    }

    /// Adds the class of each object that a value of `shape` may be to
    /// `found`.
    fn classes_in(&self, shape: Option<Shape>, found: &mut Vec<ClassId>) {
        match shape {
            Some(Shape::Instance(class, _)) => {
                found.extend(self.classes.borrow().ids.get(&class).copied());
            }
            Some(Shape::Union(shapes)) => {
                for shape in shapes {
                    self.classes_in(Some(shape), found);
                }
            }
            _ => {}
        }
    }

    /// The comparison `c`, with `ops` and the lowered `operands`, where two
    /// operands may be objects of the same dataclass. The `__eq__` of the
    /// class compares them, and `!=` is `not` of `==`, as in CPython.
    pub(super) fn dataclass_compare(
        &self,
        c: &ast::ExprCompare,
        ops: Vec<CmpOp>,
        operands: Vec<(Expr, &ast::Expr)>,
    ) -> Result<Expr, Error> {
        let (Ok([op]), Ok([(left, _), (right, _)])) =
            (<[CmpOp; 1]>::try_from(ops), <[_; 2]>::try_from(operands))
        else {
            return Err(unsupported(c, "a chain of comparisons of dataclasses"));
        };
        if !matches!(op, CmpOp::Eq | CmpOp::Ne) {
            return Err(unsupported(c, "an order of objects"));
        }
        let site = Some(self.site(c.range()));
        // An object may be null, which `__eq__` does not take.
        let eq = match (left.rep(), right.rep()) {
            (Rep::Object(a) | Rep::ObjectOrNone(a), Rep::Object(b) | Rep::ObjectOrNone(b))
                if a == b =>
            {
                let rep = Rep::ObjectOrNone(a);
                let coerced = |value| self.coerce(value, rep).expect("an object or None");
                Expr::Call {
                    func: self.eq_or_none(a)?,
                    args: vec![coerced(left), coerced(right)],
                    order: None,
                    rep: Rep::Bool,
                    site,
                    check: None,
                }
            }
            _ => return Err(unsupported(c, "== of a union with a dataclass")),
        };
        Ok(match op {
            CmpOp::Ne => Expr::Not(Box::new(eq)),
            _ => eq,
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
        let (def, fields) = {
            let classes = self.classes.borrow();
            let declared = classes.get(class);
            (declared.def, declared.fields.clone())
        };
        let object = Rep::Object(class);
        let func = self.reserve(vec![object, object], Rep::Bool);
        // A field may hold an object of the class itself.
        self.classes.borrow_mut().get_mut(class).eq = Some(func);
        let line = 10 + fields.len().max(1);
        let site = self.add_site(
            "<string>\n__eq__".to_owned(),
            line,
            format!("  File \"<string>\", line {line}, in __eq__\n"),
        );
        let other = || Expr::Load {
            place: Place::Local(VarId::new(1)),
            rep: object,
            check: None,
        };
        let shapes = class_info(self.model(), def)
            .and_then(|info| info.dataclass_fields)
            .expect("a dataclass that the checker knows");
        let mut same = None;
        for (i, (((_, rep), node), checked)) in fields
            .iter()
            .zip(dataclass_fields(def))
            .zip(shapes)
            .enumerate()
        {
            let field = u32::try_from(i).expect("fewer than 2^32 fields");
            let theirs = Expr::Field {
                object: Box::new(other()),
                class,
                field,
                rep: *rep,
            };
            let eq = self.field_eq(
                self.own_field(class, field),
                theirs,
                site,
                node,
                checked.shape,
            )?;
            same = Some(match same {
                None => eq,
                Some(before) => Expr::Logic {
                    op: LogicOp::And,
                    left: Box::new(before),
                    right: Box::new(eq),
                },
            });
        }
        let body = Expr::Logic {
            op: LogicOp::Or,
            left: Box::new(Expr::Is(Box::new(own(class)), Box::new(other()))),
            right: Box::new(same.unwrap_or(Expr::Bool(true))),
        };
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
        let dataclass = |class: ClassId| self.classes.borrow().get(class).kind == Kind::Dataclass;
        // An object may be null, which `__eq__` does not take.
        let func = match rep {
            Rep::Object(class) | Rep::ObjectOrNone(class) if dataclass(class) => {
                Some(self.eq_or_none(class)?)
            }
            _ => None,
        };
        if let Some(func) = func {
            return Ok(Expr::Call {
                func,
                args: vec![ours, theirs],
                order: None,
                rep: Rep::Bool,
                site: Some(site),
                check: None,
            });
        }
        if rep == Rep::Uniform {
            let mut found = Vec::new();
            self.classes_in(Some(shape), &mut found);
            if found.into_iter().any(dataclass) {
                return Err(unsupported(
                    node,
                    "== of a field of a union with a dataclass",
                ));
            }
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
    /// `__eq__` of the class compares two objects. The function has no
    /// frame.
    fn eq_or_none(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).eq_or_none {
            return Ok(func);
        }
        let eq = self.class_eq(class)?;
        let param = |i| Expr::Load {
            place: Place::Local(VarId::new(i)),
            rep: Rep::ObjectOrNone(class),
            check: None,
        };
        let narrow = |i| Expr::Narrow {
            value: Box::new(param(i)),
            rep: Rep::Object(class),
        };
        let body = Expr::IfElse {
            cond: Box::new(Expr::Logic {
                op: LogicOp::Or,
                left: Box::new(Expr::IsNone(Box::new(param(0)))),
                right: Box::new(Expr::IsNone(Box::new(param(1)))),
            }),
            then: Box::new(Expr::Is(Box::new(param(0)), Box::new(param(1)))),
            else_: Box::new(Expr::Call {
                func: eq,
                args: vec![narrow(0), narrow(1)],
                order: None,
                rep: Rep::Bool,
                site: Some(Site::NONE),
                check: None,
            }),
        };
        let or_none = Rep::ObjectOrNone(class);
        let func = self.generated(
            vec![or_none, or_none],
            Rep::Bool,
            vec![Stmt::Return(Some(body))],
        );
        self.classes.borrow_mut().get_mut(class).eq_or_none = Some(func);
        Ok(func)
    }

    /// The class that `name` reads, and `None` when it reads no class of
    /// the module.
    pub(super) fn class_named(&self, name: &ast::ExprName) -> Option<ClassId> {
        let id = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(id))
        {
            return None;
        }
        self.classes.borrow().names.get(id).copied()
    }

    /// Refuses an annotation of `annotations` that reads a class before its
    /// `class` statement. CPython 3.13 evaluates an annotation without
    /// quotes when the statement that has it runs, and raises `NameError`
    /// there, while CPython 3.14 does not evaluate it.
    pub(super) fn check_annotations<'e>(
        &self,
        annotations: impl Iterator<Item = &'e ast::Expr>,
    ) -> Result<(), Error> {
        struct Names<'e>(Vec<&'e ast::ExprName>);
        impl<'e> Visitor<'e> for Names<'e> {
            fn visit_expr(&mut self, expr: &'e ast::Expr) {
                if let ast::Expr::Name(name) = expr {
                    self.0.push(name);
                }
                walk_expr(self, expr);
            }
        }
        let mut names = Names(Vec::new());
        for annotation in annotations {
            names.visit_expr(annotation);
        }
        for name in names.0 {
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

    /// The members of `def` in the order of the class, when `def` has the
    /// form of an `Enum` that the compiler takes. `mro` and `checked` are
    /// what the checker gives for the class.
    fn enum_members(
        &self,
        def: &ast::StmtClassDef,
        mro: &[CheckerClass],
        checked: Option<&[(String, Shape)]>,
    ) -> Result<Vec<Member>, Error> {
        let is_enum = mro.len() == 3
            && mro
                .get(1)
                .is_some_and(|base| base.known == Some(KnownClass::Enum))
            && def.decorator_list.is_empty()
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
        // CPython gives `auto()` the greatest value before it, plus 1.
        let mut greatest: Option<i32> = None;
        for (i, stmt) in def.body.iter().enumerate() {
            let (target, value) = match stmt {
                ast::Stmt::Pass(_) => continue,
                ast::Stmt::Expr(e) if e.value.is_ellipsis_literal_expr() => continue,
                ast::Stmt::Expr(e) if i == 0 && e.value.is_string_literal_expr() => continue,
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
                ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
                    value: ast::Number::Int(n),
                    ..
                }) => int_literal(&**value, n.as_i64())?,
                ast::Expr::UnaryOp(ast::ExprUnaryOp {
                    op: ast::UnaryOp::USub,
                    operand,
                    ..
                }) => match &**operand {
                    ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
                        value: ast::Number::Int(n),
                        ..
                    }) => int_literal(&**value, n.as_i64().map(|n| -n))?,
                    _ => return Err(unsupported(&**value, "a member of an Enum with this value")),
                },
                ast::Expr::StringLiteral(_) => self.expr(value)?,
                _ => return Err(unsupported(&**value, "a member of an Enum with this value")),
            };
            if let Expr::Int(n) = value {
                greatest = Some(greatest.map_or(n, |greatest| greatest.max(n)));
            }
            if let Some(first) = members.first()
                && std::mem::discriminant(&first.value) != std::mem::discriminant(&value)
            {
                return Err(unsupported(stmt, "an Enum with values of two types"));
            }
            if members
                .iter()
                .any(|member| same_value(&member.value, &value))
            {
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
                        (Shape::IntLit(a), Expr::Int(b)) => *a == i64::from(*b),
                        (Shape::StrLit(a), Expr::Str(b)) => a == b,
                        _ => false,
                    }
            });
        if !agrees {
            return Err(unsupported(
                def,
                "an Enum whose values the checker computes otherwise",
            ));
        }
        Ok(members)
    }

    /// Returns `true` if `expr` reads the class `known` of the standard
    /// library through a name that an import binds before `def`, as `Enum`
    /// or `enum.Enum` does, `false` otherwise.
    fn names_known(&self, def: &ast::StmtClassDef, expr: &ast::Expr, known: KnownClass) -> bool {
        let name = match expr {
            ast::Expr::Name(name) => name,
            ast::Expr::Attribute(ast::ExprAttribute { value, .. }) => match &**value {
                ast::Expr::Name(name) => name,
                _ => return false,
            },
            _ => return false,
        };
        self.imported_before(name, def)
            && matches!(
                expr_shape(self.model(), expr),
                Some(Shape::Class(class)) if class.known == Some(known)
            )
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

    /// Checks that `def`, a class with a decorator, has the form of a
    /// dataclass that the compiler takes. `info` is what the checker gives
    /// for the class.
    fn dataclass_form(&self, def: &ast::StmtClassDef, info: &ClassInfo) -> Result<(), Error> {
        if def
            .arguments
            .as_deref()
            .is_some_and(|arguments| !arguments.is_empty())
            || def.type_params.is_some()
            || info.mro.len() != 2
        {
            return Err(unsupported(def, "a dataclass with a base"));
        }
        let Some(checked) = &info.dataclass_fields else {
            return Err(unsupported(def, "this class"));
        };
        let mut fields = Vec::new();
        for (i, stmt) in def.body.iter().enumerate() {
            match stmt {
                ast::Stmt::Pass(_) => {}
                ast::Stmt::Expr(e) if e.value.is_ellipsis_literal_expr() => {}
                ast::Stmt::Expr(e) if i == 0 && e.value.is_string_literal_expr() => {}
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
        // The checker leaves out of the fields an annotation such as
        // `ClassVar`, which the text alone does not tell apart.
        let agrees = checked.len() == fields.len()
            && checked.iter().zip(&fields).all(|(field, (name, default))| {
                field.name == *name && field.has_default == *default
            });
        if !agrees {
            return Err(unsupported(
                def,
                "a dataclass whose fields the checker computes otherwise",
            ));
        }
        Ok(())
    }

    /// Returns `true` if `expr` reads the function `dataclass` of the
    /// standard library through a name that an import binds before `def`,
    /// as `dataclass` or `dataclasses.dataclass` does, `false` otherwise.
    fn names_dataclass(&self, def: &ast::StmtClassDef, expr: &ast::Expr) -> bool {
        let name = match expr {
            ast::Expr::Name(name) => name,
            ast::Expr::Attribute(ast::ExprAttribute { value, .. }) => match &**value {
                ast::Expr::Name(name) => name,
                _ => return false,
            },
            _ => return false,
        };
        self.imported_before(name, def)
            && matches!(
                expr_shape(self.model(), expr),
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

/// Returns `true` if `a` and `b` are the same literal, `false` otherwise.
fn same_value(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Int(a), Expr::Int(b)) => a == b,
        (Expr::Str(a), Expr::Str(b)) => a == b,
        _ => false,
    }
}

/// The object in the first parameter of a function of the compiler for an
/// object of `class`.
fn own(class: ClassId) -> Expr {
    Expr::Load {
        place: Place::Local(VarId::new(0)),
        rep: Rep::Object(class),
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
