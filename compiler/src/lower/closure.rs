//! Functions as values: the name of a function of the module.

use ruff_python_ast as ast;

use super::class::param;
use super::{Lowering, unsupported};
use crate::Error;
use crate::ir::{Binding, CmpOp, Expr, FuncId, Place, Rep, Stmt};

impl Lowering<'_, '_> {
    /// The value of the function of the module that `name` reads. A name
    /// with more than one `def` has the function of the last `def` that
    /// ran.
    pub(super) fn func_value(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let id = name.id.as_str();
        let redefined = self.funcs.borrow().redefined.get(id).cloned();
        let Some((current, defs)) = redefined else {
            let func = *self
                .funcs
                .borrow()
                .names
                .get(id)
                .expect("a function of the module");
            let value = self.value_of(func);
            return self.after(Binding::Func(func), name, value);
        };
        let values: Vec<Expr> = defs.iter().map(|func| self.value_of(*func)).collect();
        let rep = values.first().expect("a name with defs").rep();
        if values.iter().any(|value| value.rep() != rep) {
            return Err(unsupported(name, "a name whose functions have two types"));
        }
        // The read of the global raises `NameError` before any `def` of the
        // name runs.
        let load_current = || Expr::Load {
            place: Place::Global(current),
            rep: Rep::Int,
            check: None,
        };
        let mut values = values.into_iter().enumerate().rev();
        let (_, last) = values.next().expect("a name with defs");
        let value = values.fold(last, |else_, (place, value)| Expr::IfElse {
            cond: Box::new(Expr::Compare(
                Box::new(load_current()),
                vec![(
                    CmpOp::Eq,
                    Expr::Int(i32::try_from(place).expect("few defs")),
                )],
            )),
            then: Box::new(value),
            else_: Box::new(else_),
        });
        let check = self.global_check(name)?;
        Ok(match check {
            Some(check) => Expr::After {
                binding: Binding::Global(current),
                check,
                value: Box::new(value),
            },
            None => value,
        })
    }

    /// The value of `func`, a function of the module, whose code calls the
    /// function with the arguments of the call of the value.
    fn value_of(&self, func: FuncId) -> Expr {
        let sig = self.funcs.borrow().get(func).clone();
        let ty = self.func_type(sig.params.clone(), sig.ret);
        let code = *self.values.borrow_mut().entry(func).or_insert_with(|| {
            let args = sig
                .params
                .iter()
                .enumerate()
                .map(|(i, rep)| param(i, *rep))
                .collect();
            // The function of the module checks the limit of the recursion
            // and has the frame of its code.
            let call = Expr::Call {
                func,
                args,
                order: None,
                rep: sig.ret,
                site: None,
                check: None,
            };
            let body = match sig.ret {
                Rep::None => vec![Stmt::Expr(call), Stmt::Return(None)],
                _ => vec![Stmt::Return(Some(call))],
            };
            let code = self.reserve(sig.params.clone(), sig.ret);
            self.define_value(code, ty, body);
            code
        });
        Expr::Closure { func: code, ty }
    }
}
