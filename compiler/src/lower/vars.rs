//! Reads and writes of variables, with the check that a name has a value.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{
    Shape, expr_shape, is_definitely_bound, is_implicit_global,
};

use super::{Lowering, coerce, unbound_local, unsupported};
use crate::Error;
use crate::ir::{Check, Expr, Place, Rep, Undefined};

impl<'a> Lowering<'a, '_> {
    /// The variable that `target` names, with `value` as a value of its
    /// rep.
    pub(super) fn target(&self, target: &ast::Expr, value: Expr) -> Result<(Place, Expr), Error> {
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
            Some((place, rep)) => match coerce(value, rep) {
                Some(value) => Ok((place, value)),
                None => Err(unsupported(target, "a variable with values of two types")),
            },
            // The checker gives the value a type that no rep holds.
            None => Err(unsupported(target, "a variable of this type")),
        }
    }

    /// The value of the variable that `name` reads.
    pub(super) fn load(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let id = name.id.as_str();
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
            return self.narrowed(name, load);
        }
        let global = self.vars.borrow().find(id);
        let Some((var, rep)) = global else {
            if self.imports.borrow().ids.contains_key(id) {
                return Err(unsupported(name, "a name of an import as a value"));
            }
            if self.funcs.borrow().names.contains_key(id) {
                return Err(unsupported(name, "a function as a value"));
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
            let narrowed = self.narrowed(name, load)?;
            if narrowed.rep() != rep {
                return Err(unsupported(
                    name,
                    "a read of a global of a union that a test narrows",
                ));
            }
            return Ok(narrowed);
        }
        self.narrowed(name, load)
    }

    /// `load`, the read `name` of a variable, as a value of the type that
    /// the checker gives to the read. The type is narrower than the type
    /// of a variable of a union after a test of the value.
    fn narrowed(&self, name: &ast::ExprName, load: Expr) -> Result<Expr, Error> {
        let rep = load.rep();
        if !matches!(rep, Rep::StrOrNone | Rep::Uniform) {
            return Ok(load);
        }
        let narrowed = match expr_shape(self.model(), &ast::Expr::Name(name.clone())) {
            Some(Shape::None) => Rep::None,
            shape => self
                .rep_of(shape)
                .ok_or_else(|| unsupported(name, "a variable of this type"))?,
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

    /// The site where the read of the global `name` raises `NameError`, and
    /// `None` when the read always finds a value. A function may run before
    /// the module assigns the global, so every read in a function checks.
    pub(super) fn global_check(&self, name: &ast::ExprName) -> Result<Option<Check>, Error> {
        if self.scope.borrow().is_none() && is_definitely_bound(self.model(), name) {
            return Ok(None);
        }
        if is_implicit_global(self.model(), name.id.as_str()) {
            // Without a value, the read takes the builtin of the name.
            return Err(unsupported(name, "a name that may be a builtin"));
        }
        Ok(Some(Check {
            site: self.site(name.range()),
            error: Undefined::Global(Box::new(self.name_error(name.id.as_str()))),
        }))
    }
}
