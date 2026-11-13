//! Reads and writes of variables, with the check that a name has a value.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{
    Shape, expr_shape, is_definitely_bound, is_implicit_global,
};

use super::name_error::may_suggest;
use super::{Lowering, unbound_local, unsupported};
use crate::Error;
use crate::ir::{Binding, Check, Expr, Place, Rep, Undefined};

impl<'a> Lowering<'a, '_> {
    /// The variable that `target` names, with `value` as a value of its
    /// rep.
    pub(super) fn target(&self, target: &ast::Expr, value: Expr) -> Result<(Place, Expr), Error> {
        let (place, rep) = self.place(target, value.rep())?;
        Ok((
            place,
            self.coerce(value, rep)
                .expect("a variable that holds the value"),
        ))
    }

    /// The variable that `target` names, which holds the values of `value`,
    /// together with its rep.
    pub(super) fn place(&self, target: &ast::Expr, value: Rep) -> Result<(Place, Rep), Error> {
        let ast::Expr::Name(name) = target else {
            return Err(unsupported(target, "this target"));
        };
        let name = name.id.as_str();
        let found = match self.scope.borrow().as_ref() {
            Some(scope) if scope.is_local(name) => scope
                .vars
                .find(name)
                .map(|(id, rep)| (Place::Local(id), rep)),
            _ => self
                .vars
                .borrow()
                .find(name)
                .map(|(id, rep)| (Place::Global(id), rep)),
        };
        match found {
            Some((place, rep)) if rep == value || self.holds(rep, value) => Ok((place, rep)),
            Some(_) => Err(unsupported(target, "a variable with values of two types")),
            // The checker gives the value a type that no rep holds.
            None => Err(unsupported(target, "a variable of this type")),
        }
    }

    /// The value of the variable that `name` reads.
    pub(super) fn load(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let id = name.id.as_str();
        if let Some(capture) = self.captured(name) {
            return self.narrowed_name(name, capture);
        }
        let local = match self.scope.borrow().as_ref() {
            Some(scope) if scope.is_local(id) => Some(
                scope
                    .vars
                    .find(id)
                    .ok_or_else(|| unsupported(name, "a variable of this type"))?,
            ),
            _ => None,
        };
        if let Some((var, rep)) = local {
            // An example of a docstring that raises leaves the names that it
            // would assign without a value, and they are names of a module
            // for the examples after it.
            let doctest = self.in_doctest();
            let check = if !doctest && is_definitely_bound(self.model(), name) {
                None
            } else {
                if let Some(scope) = self.scope.borrow_mut().as_mut() {
                    scope.vars.get_mut(var).flagged = true;
                }
                let error = if doctest {
                    Undefined::Global(Box::new(self.name_error(id)))
                } else {
                    Undefined::Local(unbound_local(id))
                };
                Some(Check {
                    site: self.site(name.range()),
                    error,
                })
            };
            let load = Expr::Load {
                place: Place::Local(var),
                rep,
                check,
            };
            return self.narrowed_name(name, load);
        }
        let global = self.vars.borrow().find(id);
        let Some((var, rep)) = global else {
            if self.imports.borrow().ids.contains_key(id) {
                return Err(unsupported(name, "a name of an import as a value"));
            }
            if self.funcs.borrow().names.contains_key(id) {
                return self.func_value(name);
            }
            if self.classes.borrow().names.contains_key(id) {
                return Err(unsupported(name, "a class as a value"));
            }
            return Err(unsupported(name, "a name that the module does not assign"));
        };
        let check = self.global_check(name)?;
        if check.is_some() {
            self.vars.borrow_mut().get_mut(var).flagged = true;
        }
        let load = Expr::Load {
            place: Place::Global(var),
            rep,
            check,
        };
        // A call between the test of the value and the read may assign a
        // global that a function declares.
        if self.assigned_globals.borrow().contains(id) {
            let narrowed = self.narrowed_name(name, load)?;
            if narrowed.rep() != rep {
                return Err(unsupported(
                    name,
                    "a read of a global of a union that a test narrows",
                ));
            }
            return Ok(narrowed);
        }
        self.narrowed_name(name, load)
    }

    /// [`Self::narrowed`] of the read `name`, which builds the node of the
    /// read only for a union.
    fn narrowed_name(&self, name: &ast::ExprName, load: Expr) -> Result<Expr, Error> {
        if !self.may_narrow(load.rep()) {
            return Ok(load);
        }
        self.narrowed(&ast::Expr::Name(name.clone()), load)
    }

    /// Returns `true` if the checker may give a narrower type to a value
    /// of `rep` after a test, as it does for a union and for a class with
    /// subclasses, `false` otherwise.
    fn may_narrow(&self, rep: Rep) -> bool {
        is_union(rep)
            || rep
                .class()
                .is_some_and(|class| self.descendants(class).len() > 1)
    }

    /// `load`, the read `node` of a variable or of a field, as a value of
    /// the type that the checker gives to the read. The type is narrower
    /// than the type of a variable of a union, or of a class with
    /// subclasses, after a test of the value.
    pub(super) fn narrowed(&self, node: &ast::Expr, load: Expr) -> Result<Expr, Error> {
        let rep = load.rep();
        if !self.may_narrow(rep) {
            return Ok(load);
        }
        let narrowed = match expr_shape(self.model(), node) {
            Some(Shape::None) => Rep::None,
            // The checker gives an intersection, as `object & ~bool`, after
            // a test that fails, and the rep of the variable holds it.
            Some(Shape::Other(_)) => rep,
            shape => self
                .rep_of(shape)
                .ok_or_else(|| unsupported(node, "a variable of this type"))?,
        };
        Ok(if narrowed == rep {
            load
        } else {
            Expr::Narrow {
                value: Box::new(load),
                rep: narrowed,
            }
        })
    }

    /// `value`, which reads the global `name` that `binding` gives. A read
    /// before the binding raises `NameError`.
    pub(super) fn after(
        &self,
        binding: Binding,
        name: &ast::ExprName,
        value: Expr,
    ) -> Result<Expr, Error> {
        Ok(match self.global_check(name)? {
            Some(check) => Expr::After {
                binding,
                check,
                value: Box::new(value),
            },
            None => value,
        })
    }

    /// The site where the read of the global `name` raises `NameError`, and
    /// `None` when the read always finds a value. A function may run before
    /// the module assigns the global, so every read in a function checks.
    pub(super) fn global_check(&self, name: &ast::ExprName) -> Result<Option<Check>, Error> {
        if self.scope.borrow().is_none() && is_definitely_bound(self.model(), name) {
            return Ok(None);
        }
        // A name of the namespace of a class hides the global of the name in
        // the body of the class.
        if self.scope.borrow().as_ref().is_some_and(|scope| {
            scope
                .namespace
                .iter()
                .any(|bound| *bound == name.id.as_str())
        }) {
            return Err(unsupported(
                name,
                "a read of a name of the class in its body",
            ));
        }
        // The body of a class runs once, at the `class` statement, and the
        // checker does not tell which names of the module have a value there.
        let class = self.scope.borrow().as_ref().and_then(|scope| scope.class);
        if let Some(start) = class
            && self.bound_before(name.id.as_str(), start)
        {
            return Ok(None);
        }
        if is_implicit_global(self.model(), name.id.as_str()) {
            // Without a value, the read takes the builtin of the name.
            return Err(unsupported(name, "a name that may be a builtin"));
        }
        // The `NameError` in the body of a class suggests a name of the
        // namespace of the class, which the compiler does not keep.
        if let Some(scope) = self.scope.borrow().as_ref()
            && scope
                .namespace
                .iter()
                .any(|bound| may_suggest(name.id.as_str(), bound))
        {
            return Err(unsupported(name, "a name near a name of the class"));
        }
        // An object of an `Enum` has attributes such as `_value_` that the
        // compiler does not list.
        if name.id.starts_with('_')
            && let Some(scope) = self.scope.borrow().as_ref()
            && let Some((_, Rep::Object(class))) = scope.vars.find("self")
            && self.is_enum(class)
        {
            return Err(unsupported(name, "a name with _ in a method of an Enum"));
        }
        // The suggestion of `self.x` depends on whether `__init__` has
        // assigned the field `x` yet.
        if let Some(scope) = self.scope.borrow().as_ref()
            && scope.name == "__init__"
            && let Some((_, Rep::Object(class))) = scope.vars.find("self")
            && self
                .classes
                .borrow()
                .get(class)
                .fields
                .iter()
                .any(|(field, _)| field == name.id.as_str())
        {
            return Err(unsupported(name, "the name of a field in __init__"));
        }
        Ok(Some(Check {
            site: self.site(name.range()),
            error: Undefined::Global(Box::new(self.name_error(name.id.as_str()))),
        }))
    }
}

/// Returns `true` if a value of `rep` may have one of several types, which
/// a test of the value narrows, `false` otherwise.
fn is_union(rep: Rep) -> bool {
    matches!(
        rep,
        Rep::StrOrNone | Rep::Uniform | Rep::ObjectOrNone(_) | Rep::ListOrNone(_)
    )
}
