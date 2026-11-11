//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::DefId;

use super::{Loaded, Lowering, Sig, coerce, unsupported};
use crate::Error;
use crate::cpython::names;
use crate::ir::{Case, CodePointSet, Expr, FuncId, Rep, Site};

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

    /// A call of a method of a builtin type, which the prelude defines with
    /// the receiver as the first parameter. A parameter that holds `None`
    /// stands for a parameter of the stub with a default, and a call
    /// without the argument passes `None`.
    pub(super) fn method_call(
        &self,
        call: &ast::ExprCall,
        attr: &ast::ExprAttribute,
    ) -> Result<Expr, Error> {
        let receiver = self.expr(&attr.value)?;
        let name = attr.attr.as_str();
        // The module `str` of the prelude defines each method of `str`
        // that it has under the name of the method, among other functions.
        let module = match receiver.rep() {
            Rep::Str if !name.starts_with('_') && names::STR_ATTRIBUTES.contains(&name) => "str",
            _ => return Err(unsupported(call, "a call of this method")),
        };
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(
                call,
                "a call of a method with keyword arguments",
            ));
        }
        let func = self
            .prelude_named(module, name)?
            .ok_or_else(|| unsupported(call, "a call of this method"))?;
        let params = self.funcs.borrow().get(func).params.clone();
        let mut given = vec![receiver];
        for arg in &call.arguments.args {
            given.push(self.expr(arg)?);
        }
        if given.len() > params.len() {
            return Err(unsupported(
                call,
                "a call of this method with these arguments",
            ));
        }
        let mut given = given.into_iter();
        let args = params
            .iter()
            .map(|rep| {
                prelude_arg(given.next().unwrap_or(Expr::None), *rep)
                    .ok_or_else(|| unsupported(call, "an argument of this type"))
            })
            .collect::<Result<_, _>>()?;
        let site = self.call_site(func, call);
        Ok(self.prelude_call_of(func, args, site))
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
        let funcs = self.funcs.borrow();
        let sig = funcs.get(func);
        debug_assert!(
            args.iter().map(Expr::rep).eq(sig.params.iter().copied()),
            "the arguments of {}",
            sig.def
                .map_or("a function of the compiler", |def| def.name.as_str())
        );
        Expr::Call {
            func,
            order: None,
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
        let Some((name, def)) = unit.defs.get_key_value(name) else {
            return Ok(None);
        };
        let (key, def) = ((file, *name), *def);
        if let Some(func) = self.prelude_funcs.borrow().get(&key) {
            return Ok(Some(*func));
        }
        let outer = self.unit.replace(unit);
        let signature = self.signature(def);
        self.unit.set(outer);
        let (id, params, ret) = signature?;
        let sig = Sig {
            unit,
            def: Some(def),
            defaults: params.iter().map(|_| None).collect(),
            doctest: None,
            params,
            ret,
        };
        let func = self.funcs.borrow_mut().add(id, sig);
        self.prelude_funcs.borrow_mut().insert(key, func);
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
            "natural_power" => arg().zip(arg()).map(|(base, exp)| Expr::IntPow {
                base,
                exp,
                site: self.site(call.range()),
            }),
            "code_point" => arg()
                .zip(arg())
                .map(|(value, index)| Expr::CodePoint { value, index }),
            _ if let Some(case) = Case::of(name) => {
                arg().zip(arg()).map(|(operand, index)| Expr::CaseAt {
                    case,
                    operand,
                    index,
                })
            }
            _ if let Some(set) = CodePointSet::of(name) => {
                arg().map(|operand| Expr::CodePointIs { set, operand })
            }
            "write_err" => arg().map(Expr::WriteErr),
            "decimal_value" => arg().map(Expr::DecimalValue),
            "parse_float" => arg().map(Expr::ParseFloat),
            "stop" => arg().map(|text| Expr::Stop {
                text,
                site: self.site(call.range()),
            }),
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

/// `value` as an argument of a parameter of `rep` of a function of the
/// prelude, and `None` when the parameter does not take it. A parameter of
/// `int`, or of a union with `int`, takes a `bool` as the `int` it is,
/// because the prelude reads the union as an `int` after a test. Any other
/// parameter keeps the `bool`.
pub(super) fn prelude_arg(value: Expr, rep: Rep) -> Option<Expr> {
    match (value.rep(), rep) {
        (Rep::Bool, Rep::Int | Rep::Uniform) => coerce(Expr::BoolAsInt(Box::new(value)), rep),
        _ => coerce(value, rep),
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
