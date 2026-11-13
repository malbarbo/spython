//! The `hash` of a value, which the tables of `dict` and `set` in the
//! prelude ask for.

use std::collections::HashSet;

use ruff_text_size::Ranged;

use ty_python_semantic::types::lowering::Shape;

use super::class::{Kind, generated_call, param, widen};
use super::{Error, Lowering, unsupported};
use crate::ir::{ClassId, Expr, FuncId, Rep, Site, Stmt};

/// Where the `hash` of an object of a class comes from.
enum Source {
    /// The method `__hash__` of the program.
    Method(FuncId),
    /// The fields of a frozen dataclass, as a tuple of them.
    Fields(ClassId),
    /// The name of a member of an `Enum`.
    Name(ClassId),
    /// None: the object has no `hash`.
    Unhashable,
    /// The identity of the object, which a class without `__eq__` and
    /// without `__hash__` takes from `object`.
    Identity,
}

impl Lowering<'_, '_> {
    /// The `hash` of `value` at `node`, as CPython gives it. A `__hash__`
    /// of the program raises at `site`.
    pub(super) fn hash_value(
        &self,
        node: &impl Ranged,
        value: Expr,
        site: Site,
    ) -> Result<Expr, Error> {
        match value.rep() {
            Rep::Int | Rep::Bool | Rep::Float | Rep::None | Rep::Str | Rep::StrOrNone => {
                Ok(Expr::Hash(Box::new(value)))
            }
            Rep::Object(class) => Ok(generated_call(
                self.class_hash(class),
                vec![value],
                Rep::Hash,
                site,
            )),
            rep @ Rep::ObjectOrNone(class) => {
                let object = Expr::Narrow {
                    value: Box::new(param(0, rep)),
                    rep: Rep::Object(class),
                };
                let body = vec![
                    Stmt::If {
                        cond: Expr::IsNone(Box::new(param(0, rep))),
                        then: vec![Stmt::Return(Some(Expr::Hash(Box::new(Expr::None))))],
                        else_: Vec::new(),
                    },
                    Stmt::Return(Some(generated_call(
                        self.class_hash(class),
                        vec![object],
                        Rep::Hash,
                        Site::NONE,
                    ))),
                ];
                let func = self.generated(vec![rep], Rep::Hash, body);
                Ok(generated_call(func, vec![value], Rep::Hash, site))
            }
            Rep::Uniform => Ok(generated_call(
                self.uniform_hash()?,
                vec![value],
                Rep::Hash,
                site,
            )),
            Rep::List(_) | Rep::ListOrNone(_) | Rep::Hash | Rep::Func(_) => {
                Err(unsupported(node, "the hash of this type"))
            }
        }
    }

    /// Returns `true` if each value of `shape` has a `hash`, `false`
    /// otherwise. An object has one when each class that its class may be
    /// has one, and a frozen dataclass needs one for each field. A value
    /// of `object` may be a `dict`, a `set` or a list, which have none.
    pub(super) fn hashable(&self, shape: &Shape) -> bool {
        self.hashable_in(shape, &mut HashSet::new())
    }

    /// [`Self::hashable`], where `seen` holds the dataclasses whose fields
    /// the test already takes, as the ones that a field of a dataclass of
    /// its own class reaches again.
    fn hashable_in(&self, shape: &Shape, seen: &mut HashSet<ClassId>) -> bool {
        match shape {
            Shape::Union(shapes) => {
                return shapes.iter().all(|shape| self.hashable_in(shape, seen));
            }
            Shape::None => return true,
            Shape::Instance(class, _) if self.is_object(class) => return false,
            Shape::Dict(..) | Shape::Set(_) | Shape::List(_) => return false,
            _ => {}
        }
        match self.rep_of(Some(shape.clone())) {
            Some(Rep::Int | Rep::Bool | Rep::Float | Rep::None | Rep::Str | Rep::StrOrNone) => true,
            Some(Rep::Uniform | Rep::Object(_) | Rep::ObjectOrNone(_)) => self
                .classes_in(Some(shape.clone()))
                .into_iter()
                .all(|class| self.class_hashable(class, seen)),
            Some(Rep::List(_) | Rep::ListOrNone(_) | Rep::Hash | Rep::Func(_)) | None => false,
        }
    }

    /// Returns `true` if an object of exactly `class` has a `hash`,
    /// `false` otherwise.
    fn class_hashable(&self, class: ClassId, seen: &mut HashSet<ClassId>) -> bool {
        match self.hash_source(class) {
            Source::Method(_) | Source::Name(_) => true,
            Source::Fields(owner) => {
                let fields: Vec<Shape> = {
                    let classes = self.classes.borrow();
                    let nodes = &classes.get(owner).nodes;
                    nodes.iter().map(|(_, shape)| shape.clone()).collect()
                };
                !seen.insert(owner) || fields.iter().all(|shape| self.hashable_in(shape, seen))
            }
            Source::Unhashable | Source::Identity => false,
        }
    }

    /// The function that gives the `hash` of a uniform value. It tests
    /// the exact class of an object against each class with a `hash`,
    /// and the value of another type has the `hash` of its type.
    fn uniform_hash(&self) -> Result<FuncId, Error> {
        if let Some(func) = self.uniform_hash.get() {
            return Ok(func);
        }
        let func = self.reserve(vec![Rep::Uniform], Rep::Hash);
        self.uniform_hash.set(Some(func));
        let uniform = || param(0, Rep::Uniform);
        let mut body = Vec::new();
        for class in self.all_classes() {
            if matches!(
                self.hash_source(class),
                Source::Unhashable | Source::Identity
            ) {
                continue;
            }
            let hash = self.exact_hash(class)?;
            let object = Expr::AsExact {
                value: Box::new(uniform()),
                class,
            };
            body.push(Stmt::If {
                cond: Expr::IsExact {
                    value: Box::new(uniform()),
                    class,
                },
                then: vec![Stmt::Return(Some(generated_call(
                    hash,
                    vec![widen(object, self.self_rep(hash))],
                    Rep::Hash,
                    Site::NONE,
                )))],
                else_: Vec::new(),
            });
        }
        body.push(Stmt::Return(Some(Expr::Hash(Box::new(uniform())))));
        self.define(func, false, body);
        Ok(func)
    }

    /// Where the `hash` of an object of exactly `class` comes from, the
    /// first class of its chain that says it. A `__hash__` of the body
    /// says it. The decorator of a dataclass gives a frozen one the `hash`
    /// of its fields and takes it away from the others, and an `Enum`
    /// gives its members the `hash` of their names. A body with `__eq__`
    /// and without `__hash__` takes the `hash` away.
    fn hash_source(&self, class: ClassId) -> Source {
        let classes = self.classes.borrow();
        for class in classes.chain(class) {
            if let Some(method) = classes.own_method(class, "__hash__") {
                return Source::Method(method.func());
            }
            let eq = classes.own_method(class, "__eq__").is_some();
            match classes.get(class).kind {
                Kind::Dataclass { frozen: true } => return Source::Fields(class),
                Kind::Dataclass { frozen: false } => return Source::Unhashable,
                Kind::Enum | Kind::Plain if eq => return Source::Unhashable,
                Kind::Enum => return Source::Name(class),
                Kind::Plain => {}
            }
        }
        Source::Identity
    }

    /// The function that gives the `hash` of an object of exactly `class`.
    /// Each class whose `hash` comes from the same class shares it.
    pub(super) fn exact_hash(&self, class: ClassId) -> Result<FuncId, Error> {
        let owner = match self.hash_source(class) {
            Source::Method(func) => self.receiver(func),
            Source::Fields(owner) | Source::Name(owner) => owner,
            Source::Unhashable | Source::Identity => {
                unreachable!("a table of a class without a hash")
            }
        };
        if let Some(func) = self.classes.borrow().get(owner).own_hash {
            return Ok(func);
        }
        let (def, fields) = {
            let classes = self.classes.borrow();
            let declared = classes.get(owner);
            (declared.def, declared.fields.len())
        };
        let hash = match self.hash_source(owner) {
            // `__hash__` gives an `int`, whose `hash` takes -1 to -2.
            Source::Method(func) => Expr::Hash(Box::new(generated_call(
                func,
                vec![param(0, Rep::Object(owner))],
                Rep::Int,
                Site::NONE,
            ))),
            Source::Name(_) => Expr::Hash(Box::new(self.own_field(owner, 0))),
            _ => Expr::HashTuple(
                (0..fields)
                    .map(|field| {
                        let field = u32::try_from(field).expect("fewer than 2^32 fields");
                        self.hash_value(def, self.own_field(owner, field), Site::NONE)
                    })
                    .collect::<Result<_, _>>()?,
            ),
        };
        let func = self.generated(
            vec![Rep::Object(owner)],
            Rep::Hash,
            vec![Stmt::Return(Some(hash))],
        );
        self.classes.borrow_mut().get_mut(owner).own_hash = Some(func);
        Ok(func)
    }
}
