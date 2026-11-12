//! The `hash` of a value, which the tables of `dict` and `set` in the
//! prelude ask for.

use ruff_text_size::Ranged;

use super::class::{Kind, generated_call, param};
use super::{Error, Lowering, unsupported};
use crate::ir::{ClassId, Expr, FuncId, Rep, Site, Stmt};

/// Where the `hash` of an object of a class comes from.
enum Source {
    /// The fields of a frozen dataclass, as a tuple of them.
    Fields(ClassId),
    /// The name of a member of an `Enum`.
    Name(ClassId),
    /// None: the object has no `hash`.
    Unhashable,
}

impl Lowering<'_, '_> {
    /// The `hash` of `value` at `node`, as CPython gives it.
    pub(super) fn hash_value(&self, node: &impl Ranged, value: Expr) -> Result<Expr, Error> {
        match value.rep() {
            Rep::Int | Rep::Bool | Rep::Float | Rep::None | Rep::Str => {
                Ok(Expr::Hash(Box::new(value)))
            }
            Rep::Object(class) => Ok(generated_call(
                self.class_hash(class),
                vec![value],
                Rep::Hash,
                Site::NONE,
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
                Ok(generated_call(func, vec![value], Rep::Hash, Site::NONE))
            }
            _ => Err(unsupported(node, "the hash of this type")),
        }
    }

    /// Where the `hash` of an object of exactly `class` comes from. The
    /// decorator of a dataclass gives a frozen one the `hash` of its
    /// fields and takes it away from the others, and an `Enum` gives its
    /// members the `hash` of their names.
    fn hash_source(&self, class: ClassId) -> Source {
        let classes = self.classes.borrow();
        match classes.get(class).kind {
            Kind::Dataclass { frozen: true } => Source::Fields(class),
            Kind::Enum => Source::Name(class),
            Kind::Dataclass { frozen: false } | Kind::Plain => Source::Unhashable,
        }
    }

    /// The function that gives the `hash` of an object of exactly `class`.
    /// Each class whose `hash` comes from the same class shares it.
    pub(super) fn exact_hash(&self, class: ClassId) -> Result<FuncId, Error> {
        let owner = match self.hash_source(class) {
            Source::Fields(owner) | Source::Name(owner) => owner,
            Source::Unhashable => unreachable!("a table of a class without a hash"),
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
            Source::Name(_) => Expr::Hash(Box::new(self.own_field(owner, 0))),
            _ => Expr::HashTuple(
                (0..fields)
                    .map(|field| {
                        let field = u32::try_from(field).expect("fewer than 2^32 fields");
                        self.hash_value(def, self.own_field(owner, field))
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
