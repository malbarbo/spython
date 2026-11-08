//! Calls of the functions of the module and of the builtins.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Expr, Rep};

impl<'a> Lowering<'a, '_> {
    /// A call of a function of the module, with positional arguments.
    pub(super) fn call(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ast::Expr::Name(name) = &*call.func else {
            return Err(unsupported(call, "this call"));
        };
        let callee = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(callee))
        {
            return Err(unsupported(call, "this call"));
        }
        let Some(func) = self.funcs.borrow().ids.get(callee).copied() else {
            return Err(unsupported(call, "this call"));
        };
        // The `def` is the one binding of the name in the module, so the call
        // finds the function, or raises `NameError` before the `def` runs.
        let sig = self.funcs.borrow().get(func).clone();
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "a call with keyword arguments"));
        }
        if call.arguments.args.len() != sig.params.len() {
            return Err(unsupported(call, "a call with this number of arguments"));
        }
        let check = self.global_check(name)?;
        let args = call
            .arguments
            .args
            .iter()
            .zip(&sig.params)
            .map(|(arg, rep)| {
                let lowered = self.expr(arg)?;
                if lowered.rep() != *rep {
                    return Err(unsupported(arg, "an argument of this type"));
                }
                Ok(lowered)
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Call {
            func,
            args,
            rep: sig.ret,
            site: self.site(call.range()),
            check,
        })
    }

    pub(super) fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::None => Err(unsupported(arg, "print of None")),
                    _ => Ok(lowered),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print {
            args,
            site: self.site(call.range()),
        })
    }

    /// `len` of one `str`.
    pub(super) fn len(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of len"));
        };
        let operand = self.expr(arg)?;
        if operand.rep() != Rep::Str {
            return Err(unsupported(arg, "len of this type"));
        }
        Ok(Expr::Len {
            operand: Box::new(operand),
            site: self.site(call.range()),
        })
    }

    /// Returns `true` if `func` is the function `name` of the builtins,
    /// `false` otherwise.
    pub(super) fn is_builtin(&self, func: &ast::Expr, name: &str) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Function(def)) if def.name == name && is_builtins(self.model.db(), def.file)
        )
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}
