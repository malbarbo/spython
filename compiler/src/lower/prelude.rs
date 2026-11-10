//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::DefId;

use super::{Loaded, Lowering, Sig, unsupported};
use crate::Error;
use crate::ir::{CodePointSet, Expr, FuncId, Site};

/// Each module of the prelude that the database of `file` has, by name.
pub(super) fn load(db: &dyn Db, file: File) -> Vec<(&'static str, Loaded<'_>)> {
    let importing = ImportingFile::ResolverFile(db.program_file(file).resolver_file(db));
    ::prelude::FILES
        .iter()
        .filter_map(|(path, _)| {
            let name = path.strip_suffix(".py")?;
            let module = ModuleName::new(&format!("{}.{name}", ::prelude::PACKAGE))?;
            let file = resolve_module(db, importing, &module)?.file(db)?;
            (name != "__init__").then(|| (name, Loaded::new(db, file)))
        })
        .collect()
}

/// The stub of the intrinsics in the database of `file`.
pub(super) fn intrinsics(db: &dyn Db, file: File) -> Option<File> {
    let importing = ImportingFile::ResolverFile(db.program_file(file).resolver_file(db));
    let module = ModuleName::new(&format!("{}.intrinsics", ::prelude::PACKAGE))?;
    resolve_module(db, importing, &module)?.file(db)
}

impl<'a, 'db> Lowering<'a, 'db> {
    /// A call of the function `name` of the module `module` of the prelude,
    /// with one argument for each parameter, in the place of `node`.
    pub(super) fn prelude_call(
        &self,
        node: &impl Ranged,
        module: &str,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<Expr, Error> {
        let func = self
            .prelude_named(module, name)?
            .ok_or_else(|| unsupported(node, "a builtin that the prelude lacks"))?;
        let site = self.call_site(func, node);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// A call of the function `name` of the module `module` of the prelude
    /// at `site`, in code that the lowering adds to the program.
    pub(super) fn prelude_call_at(
        &self,
        module: &str,
        name: &str,
        args: Vec<Expr>,
        site: Site,
    ) -> Result<Expr, Error> {
        let func = self
            .prelude_named(module, name)?
            .expect("a function of the prelude of the compiler");
        Ok(self.prelude_call_of(func, args, Some(site)))
    }

    /// The function `name` of the module `module` of the prelude.
    fn prelude_named(&self, module: &str, name: &str) -> Result<Option<FuncId>, Error> {
        match self
            .prelude
            .iter()
            .find(|unit| unit.prelude == Some(module))
        {
            Some(unit) => self.prelude_func(unit.file, name),
            None => Ok(None),
        }
    }

    /// The call of `func`, a function of the prelude, with one argument for
    /// each parameter.
    fn prelude_call_of(&self, func: FuncId, args: Vec<Expr>, site: Option<Site>) -> Expr {
        let sig = self.funcs.borrow().get(func).clone();
        debug_assert!(
            args.iter().map(Expr::rep).eq(sig.params.iter().copied()),
            "the arguments of {}",
            sig.def.name
        );
        Expr::Call {
            func,
            order: (0..args.len())
                .map(|arg| u32::try_from(arg).expect("fewer than 2^32 arguments"))
                .collect(),
            args,
            rep: sig.ret,
            site,
            check: None,
        }
    }

    /// The function `name` of the module of the prelude in `file`, which
    /// the lowering lowers at the first call. `None` when
    /// `file` is not a module of the prelude, or has no such function.
    pub(super) fn prelude_func(&self, file: File, name: &str) -> Result<Option<FuncId>, Error> {
        let Some(unit) = self.prelude.iter().find(|unit| unit.file == file) else {
            return Ok(None);
        };
        let Some(def) = unit.defs.get(name).copied() else {
            return Ok(None);
        };
        let outer = self.unit.replace(unit);
        let signature = self.signature(def);
        self.unit.set(outer);
        let (id, params, ret) = signature?;
        if let Some(func) = self.funcs.borrow().ids.get(&id) {
            return Ok(Some(*func));
        }
        let sig = Sig {
            unit,
            def,
            defaults: params.iter().map(|_| None).collect(),
            doctest: None,
            params,
            ret,
        };
        let func = self.funcs.borrow_mut().add(id, sig);
        // The body comes now, so the call knows whether the function may
        // raise. A call of the function in its own body takes that it may.
        let scope = self.scope.take();
        let raises = self.raises.replace(false);
        let lowered = self.func(func);
        let raises = self.raises.replace(raises);
        *self.scope.borrow_mut() = scope;
        if let Some(flag) = self.funcs.borrow_mut().raises.get_mut(func.index()) {
            *flag = raises;
        }
        self.lowered.borrow_mut().insert(func.index(), lowered?);
        Ok(Some(func))
    }

    /// The site of the call of `func` at `node`, and `None` when the call
    /// raises nothing.
    pub(super) fn call_site(&self, func: FuncId, node: &impl Ranged) -> Option<Site> {
        let raises = self.funcs.borrow().raises.get(func.index()).copied();
        raises.unwrap_or(true).then(|| self.site(node.range()))
    }

    /// The function of the prelude that `def` names, as [`Self::prelude_func`]
    /// gives it.
    pub(super) fn prelude_def(&self, def: &DefId) -> Result<Option<FuncId>, Error> {
        self.prelude_func(def.file, &def.name)
    }

    /// The instruction of `name`, a function of the stub of the intrinsics,
    /// for `call`.
    pub(super) fn intrinsic(&self, call: &ast::ExprCall, name: &str) -> Result<Expr, Error> {
        let mut args = call
            .arguments
            .args
            .iter()
            .map(|arg| self.expr(arg))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter();
        let mut arg = || args.next().map(Box::new);
        let lowered = match name {
            "builder_new" => arg().map(|len| Expr::StrNew {
                len,
                site: self.site(call.range()),
            }),
            "builder_done" => arg().map(|builder| *builder),
            "code_point" => arg()
                .zip(arg())
                .map(|(value, index)| Expr::CodePoint { value, index }),
            "is_printable" => arg().map(|operand| Expr::CodePointIs {
                set: CodePointSet::Printable,
                operand,
            }),
            "write_err" => arg().map(Expr::WriteErr),
            "captured_output" => Some(Expr::Captured),
            "captured_exception" => Some(Expr::CapturedException),
            _ => None,
        };
        lowered.ok_or_else(|| unsupported(call, "this call of an intrinsic"))
    }

    /// Returns `true` if the lowering is in the code of the prelude,
    /// `false` in the code of the program.
    pub(super) fn in_prelude(&self) -> bool {
        self.unit.get().prelude.is_some()
    }
}

#[cfg(test)]
mod tests {
    use engine::Level;
    use engine::check::check_source;

    /// The number of functions in the program of `source`.
    fn funcs(source: &str) -> usize {
        let Ok(checked) = check_source(source, Level::Full, false) else {
            panic!("the checker rejects the program");
        };
        match crate::lower::lower(&checked.db, checked.file, "test.py", None) {
            Ok(program) => program.funcs.len(),
            Err(error) => panic!("{error:?}"),
        }
    }

    #[test]
    fn a_program_has_each_function_of_the_prelude_that_it_reaches_once() {
        assert_eq!(funcs("print('oi')\n"), 0);
        assert_eq!(
            funcs("import math\n\nprint(math.isnan(1.0), math.isnan(2.0))\n"),
            1
        );
    }
}
