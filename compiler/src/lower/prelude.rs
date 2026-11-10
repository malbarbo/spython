//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_text_size::Ranged;
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::DefId;

use super::{Loaded, Lowering, Sig, unsupported};
use crate::Error;
use crate::ir::{Expr, FuncId};

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
            .prelude
            .iter()
            .find(|unit| unit.prelude == Some(module))
            .and_then(|unit| self.prelude_func(unit.file, name).transpose())
            .ok_or_else(|| unsupported(node, "a builtin that the prelude lacks"))??;
        let sig = self.funcs.borrow().get(func).clone();
        debug_assert!(
            args.iter().map(Expr::rep).eq(sig.params.iter().copied()),
            "the arguments of {module}.{name}"
        );
        Ok(Expr::Call {
            func,
            order: (0..args.len())
                .map(|arg| u32::try_from(arg).expect("fewer than 2^32 arguments"))
                .collect(),
            args,
            rep: sig.ret,
            site: self.site(node.range()),
            check: None,
        })
    }

    /// The function `name` of the module of the prelude in `file`, which
    /// goes to the queue of the lowering at the first call. `None` when
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
            params,
            ret,
        };
        Ok(Some(self.funcs.borrow_mut().add(id, sig)))
    }

    /// The function of the prelude that `def` names, as [`Self::prelude_func`]
    /// gives it.
    pub(super) fn prelude_def(&self, def: &DefId) -> Result<Option<FuncId>, Error> {
        self.prelude_func(def.file, &def.name)
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
        match crate::lower::lower(&checked.db, checked.file, "test.py") {
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
