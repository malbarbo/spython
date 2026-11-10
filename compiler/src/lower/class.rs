//! The classes of the module. The compiler reads a fixed form of an `Enum`
//! in the text and asks the checker for the names and the types. The
//! compiled program runs no code of the base nor of the body of the class.

use std::collections::HashMap;

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::KnownClass;
use ty_python_semantic::types::lowering::{ClassId as CheckerClass, Shape, class_info, expr_shape};

use super::expr::int_literal;
use super::flow::imported_name;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Binding, ClassId, Expr, FuncId, Place, Rep, Site, Stmt, UserClass, VarId};

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
                fields: class.fields,
                mutable: false,
                members: class
                    .members
                    .into_iter()
                    .map(|member| vec![Expr::Str(member.name), member.value])
                    .collect(),
                str: class.str,
            })
            .collect()
    }
}

/// A class of the module, with what the lowering knows of it.
pub(super) struct Declared<'a> {
    pub def: &'a ast::StmtClassDef,
    /// `true` when the program asks which globals got a value first.
    pub ordered: bool,
    /// The rep of each field of an object of the class. A member of an
    /// `Enum` has its name and its value.
    pub fields: Vec<Rep>,
    pub members: Vec<Member>,
    /// The function that gives the `str` of an object, once a conversion
    /// needs it.
    pub str: Option<FuncId>,
    /// The function that gives the `repr` of an object, once a conversion
    /// needs it.
    pub repr: Option<FuncId>,
}

/// A member of an `Enum`, with its value.
pub(super) struct Member {
    pub name: String,
    pub value: Expr,
}

impl<'a> Lowering<'a, '_> {
    /// Declares the class of `def`, a `class` statement of the module.
    pub(super) fn declare_class(&self, def: &'a ast::StmtClassDef) -> Result<(), Error> {
        let name = def.name.as_str();
        if self.imports.borrow().ids.contains_key(name) {
            return Err(unsupported(&def.name, "a name of an import and of a class"));
        }
        if self.classes.borrow().names.contains_key(name) {
            return Err(unsupported(&def.name, "two classes of the same name"));
        }
        let info = class_info(self.model(), def).ok_or_else(|| unsupported(def, "this class"))?;
        let members = self.enum_members(def, &info.mro, info.enum_members.as_deref())?;
        let value = members
            .first()
            .map_or(Rep::Int, |member| member.value.rep());
        let mut classes = self.classes.borrow_mut();
        let id = ClassId::new(classes.list.len());
        classes.list.push(Declared {
            def,
            ordered: false,
            fields: vec![Rep::Str, value],
            members,
            str: None,
            repr: None,
        });
        classes.names.insert(name.to_owned(), id);
        classes.ids.insert(info.id, id);
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
    /// `object` gives. A member of an `Enum` has `name` and `value`.
    pub(super) fn object_attribute(
        &self,
        attr: &ast::ExprAttribute,
        object: Expr,
        class: ClassId,
    ) -> Result<Expr, Error> {
        let field = match attr.attr.as_str() {
            "name" => 0,
            "value" => 1,
            _ => return Err(unsupported(attr, "this attribute of an object")),
        };
        let rep = *self
            .classes
            .borrow()
            .get(class)
            .fields
            .get(field)
            .expect("a field of the class");
        Ok(Expr::Field {
            object: Box::new(object),
            class,
            field: u32::try_from(field).expect("fewer than 2^32 fields"),
            rep,
        })
    }

    /// The function that gives the `str` of an object of `class`, which a
    /// member of an `Enum` writes as `Cor.VERDE`.
    pub(super) fn class_str(&self, class: ClassId) -> FuncId {
        if let Some(func) = self.classes.borrow().get(class).str {
            return func;
        }
        let name = self.classes.borrow().get(class).def.name.to_string();
        let body = vec![Stmt::Return(Some(Expr::Concat {
            left: Box::new(Expr::Str(format!("{name}."))),
            right: Box::new(self.own_field(class, 0)),
            site: Site::NONE,
        }))];
        let func = self.generated(vec![Rep::Object(class)], Rep::Str, body);
        self.classes.borrow_mut().get_mut(class).str = Some(func);
        func
    }

    /// Makes the function of the `str` of every class of the module.
    pub(super) fn every_class_str(&self) {
        let count = self.classes.borrow().list.len();
        for class in 0..count {
            self.class_str(ClassId::new(class));
        }
    }

    /// The function that gives the `repr` of an object of `class`, which a
    /// member of an `Enum` writes as `<Cor.VERDE: 1>`, with the `repr` of
    /// its value.
    pub(super) fn class_repr(&self, class: ClassId) -> Result<FuncId, Error> {
        if let Some(func) = self.classes.borrow().get(class).repr {
            return Ok(func);
        }
        let name = self.classes.borrow().get(class).def.name.to_string();
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
        let rep = *self
            .classes
            .borrow()
            .get(class)
            .fields
            .get(field as usize)
            .expect("a field of the class");
        Expr::Field {
            object: Box::new(Expr::Load {
                place: Place::Local(VarId::new(0)),
                rep: Rep::Object(class),
                check: None,
            }),
            class,
            field,
            rep,
        }
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

    /// The checks that raise `NameError` for each class that an annotation
    /// of `annotations` reads before its `class` statement, in the order of
    /// CPython, which evaluates an annotation without quotes when the
    /// statement that has it runs.
    pub(super) fn annotation_checks<'e>(
        &self,
        annotations: impl Iterator<Item = &'e ast::Expr>,
    ) -> Result<Vec<Stmt>, Error> {
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
        let mut checks = Vec::new();
        for name in names.0 {
            let class = self.classes.borrow().names.get(name.id.as_str()).copied();
            if let Some(class) = class
                && let Some(check) = self.global_check(name)?
            {
                checks.push(Stmt::Expr(Expr::After {
                    binding: Binding::Class(class),
                    check,
                    value: Box::new(Expr::None),
                }));
            }
        }
        Ok(checks)
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
            return Err(unsupported(def, "a class other than an Enum"));
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
}

impl Lowering<'_, '_> {
    /// Returns `true` if an import in the code of the module, out of any
    /// `if` or `while`, binds `name` before `def`, `false` otherwise. The
    /// `class` statement reads the name when it runs, and the read raises
    /// `NameError` before the import. No other statement binds a name of
    /// an import.
    fn imported_before(&self, name: &ast::ExprName, def: &ast::StmtClassDef) -> bool {
        let name = name.id.as_str();
        self.unit
            .get()
            .suite
            .iter()
            .take_while(|stmt| stmt.range().end() <= def.range().start())
            .any(|stmt| match stmt {
                ast::Stmt::Import(i) => i
                    .names
                    .iter()
                    .any(|alias| imported_name(alias, false) == name),
                ast::Stmt::ImportFrom(i) => i
                    .names
                    .iter()
                    .any(|alias| imported_name(alias, true) == name),
                _ => false,
            })
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
