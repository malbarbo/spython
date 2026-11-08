//! Reads and writes of variables, with the check that a name has a value.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{is_definitely_bound, is_implicit_global};

use super::{Lowering, unbound_local, unsupported};
use crate::Error;
use crate::ir::{Check, Expr, Place, Undefined};

impl<'a> Lowering<'a, '_> {
    /// The variable that `target` names, which has to hold the rep of
    /// `value`.
    pub(super) fn target(&self, target: &ast::Expr, value: &Expr) -> Result<Place, Error> {
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
            Some((place, rep)) if rep == value.rep() => Ok(place),
            Some(_) => Err(unsupported(target, "a variable with values of two types")),
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
            let check = if is_definitely_bound(self.model, name) {
                None
            } else {
                if let Some(scope) = self.scope.borrow_mut().as_mut() {
                    scope.vars.get_mut(var).flagged = true;
                }
                Some(Check {
                    site: self.site(name.range()),
                    error: Undefined::Local(unbound_local(id)),
                })
            };
            return Ok(Expr::Load {
                place: Place::Local(var),
                rep,
                check,
            });
        }
        let global = self.vars.borrow().find(id);
        let Some((var, rep)) = global else {
            if self.funcs.borrow().ids.contains_key(id) {
                return Err(unsupported(name, "a function as a value"));
            }
            return Err(unsupported(name, "a name that the module does not assign"));
        };
        let check = self.global_check(name)?;
        if check.is_some() {
            self.vars.borrow_mut().get_mut(var).flagged = true;
        }
        Ok(Expr::Load {
            place: Place::Global(var),
            rep,
            check,
        })
    }

    /// The site where the read of the global `name` raises `NameError`, and
    /// `None` when the read always finds a value. A function may run before
    /// the module assigns the global, so every read in a function checks.
    pub(super) fn global_check(&self, name: &ast::ExprName) -> Result<Option<Check>, Error> {
        if self.scope.borrow().is_none() && is_definitely_bound(self.model, name) {
            return Ok(None);
        }
        if is_implicit_global(self.model, name.id.as_str()) {
            // Without a value, the read takes the builtin of the name.
            return Err(unsupported(name, "a name that may be a builtin"));
        }
        Ok(Some(Check {
            site: self.site(name.range()),
            error: Undefined::Global(Box::new(self.name_error(name.id.as_str()))),
        }))
    }
}
