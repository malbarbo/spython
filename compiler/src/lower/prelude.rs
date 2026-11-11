//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{DefId, Shape, function_info};

use super::{Loaded, Lowering, Sig, Unit, coerce, unsupported};
use crate::Error;
use crate::cpython::names;
use crate::ir::{Case, CodePointSet, Expr, FuncId, ListId, Rep, Site};

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
        // that it has under the name of the method, among other functions,
        // and the module `list` each method of `list`.
        let module = match receiver.rep() {
            Rep::Str if !name.starts_with('_') && names::STR_ATTRIBUTES.contains(&name) => "str",
            Rep::List(_) if names::LIST_METHODS.contains(&name) => "list",
            _ => return Err(unsupported(call, "a call of this method")),
        };
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(
                call,
                "a call of a method with keyword arguments",
            ));
        }
        let mut given = vec![receiver];
        for arg in &call.arguments.args {
            given.push(self.expr(arg)?);
        }
        let unit = self.prelude_unit(module);
        let reps: Vec<Rep> = given.iter().map(Expr::rep).collect();
        let (func, shapes) = match unit {
            Some(unit) => self.prelude_for(call, unit, name, &reps)?,
            None => None,
        }
        .ok_or_else(|| unsupported(call, "a call of this method"))?;
        let params = self.funcs.borrow().get(func).params.clone();
        if given.len() > params.len() {
            return Err(unsupported(
                call,
                "a call of this method with these arguments",
            ));
        }
        let mut given = given.into_iter();
        let args = params
            .iter()
            .zip(&shapes)
            .map(|(rep, shape)| {
                prelude_arg(given.next().unwrap_or(Expr::None), *rep, shape)
                    .ok_or_else(|| unsupported(call, "an argument of this type"))
            })
            .collect::<Result<_, _>>()?;
        let site = self.call_site(func, call);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// A call of `def`, a generic function of the prelude, from the code of
    /// the prelude, with positional arguments.
    pub(super) fn generic_call(&self, call: &ast::ExprCall, def: &DefId) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "a call with these arguments"));
        }
        let given = call
            .arguments
            .args
            .iter()
            .map(|arg| self.expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let reps: Vec<Rep> = given.iter().map(Expr::rep).collect();
        let unit = self
            .prelude
            .iter()
            .find(|unit| unit.file == def.file)
            .expect("a module of the prelude");
        let (func, shapes) = self
            .prelude_for(call, unit, &def.name, &reps)?
            .expect("a function of the prelude");
        let params = self.funcs.borrow().get(func).params.clone();
        if given.len() != params.len() {
            return Err(unsupported(call, "a call with these arguments"));
        }
        let args = given
            .into_iter()
            .zip(&params)
            .zip(&shapes)
            .map(|((value, rep), shape)| {
                prelude_arg(value, *rep, shape)
                    .ok_or_else(|| unsupported(call, "an argument of this type"))
            })
            .collect::<Result<_, _>>()?;
        let site = self.call_site(func, call);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// The `repr` of `value`, a list or a list or `None`, at `site`, which
    /// the module `list` of the prelude gives.
    pub(super) fn list_repr(&self, value: Expr, site: Site) -> Result<Expr, Error> {
        let rep = value.rep();
        let name = match rep {
            Rep::List(_) => "repr_of",
            _ => "repr_or_none",
        };
        let unit = self
            .prelude_unit("list")
            .expect("the module list of the prelude");
        let (func, _) = self
            .prelude_for(&TextRange::default(), unit, name, &[rep])?
            .expect("the repr of a list");
        Ok(self.prelude_call_of(func, vec![value], Some(site)))
    }

    /// The function that gives the `repr` of a list of the type `list`,
    /// which the write of a list calls.
    pub(super) fn list_repr_func(&self, list: ListId) -> Result<FuncId, Error> {
        let unit = self
            .prelude_unit("list")
            .expect("the module list of the prelude");
        let (func, _) = self
            .prelude_for(&TextRange::default(), unit, "repr_of", &[Rep::List(list)])?
            .expect("the repr of a list");
        if let Some(slot) = self.lists.borrow_mut().reprs.get_mut(list.index()) {
            *slot = Some(func);
        }
        Ok(func)
    }

    /// Returns `true` if `def` is a generic function of the prelude,
    /// `false` otherwise.
    pub(super) fn is_generic(&self, def: &DefId) -> bool {
        self.prelude
            .iter()
            .find(|unit| unit.file == def.file)
            .and_then(|unit| unit.defs.get(def.name.as_str()))
            .is_some_and(|def| def.type_params.is_some())
    }

    /// The module `module` of the prelude.
    fn prelude_unit(&self, module: &str) -> Option<&'a Unit<'a, 'db>> {
        self.prelude
            .iter()
            .find(|unit| unit.prelude == Some(module))
    }

    /// The function `name` of the module `module` of the prelude.
    fn prelude_named(&self, module: &str, name: &str) -> Result<Option<FuncId>, Error> {
        match self.prelude_unit(module) {
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

    /// The function `name` of `unit`, a module of the prelude, for a call
    /// at `node` with arguments of `args`, with the declared type of each
    /// parameter. A generic function gives the copy for the reps that the
    /// arguments give to its type variables. `None` when the module has no
    /// such function.
    fn prelude_for(
        &self,
        node: &impl Ranged,
        unit: &'a Unit<'a, 'db>,
        name: &str,
        args: &[Rep],
    ) -> Result<Option<(FuncId, Vec<Shape>)>, Error> {
        let Some(def) = unit.defs.get(name) else {
            return Ok(None);
        };
        let info = function_info(unit.model, def).ok_or_else(|| unsupported(node, "this call"))?;
        let shapes: Vec<Shape> = info.params.into_iter().map(|param| param.shape).collect();
        let names: Vec<String> = def
            .type_params
            .iter()
            .flat_map(|params| &params.type_params)
            .map(|param| param.name().to_string())
            .collect();
        let types = type_map(&names, &shapes, args, |list| self.list_item(list))
            .ok_or_else(|| unsupported(node, "a call of a generic function with these types"))?;
        let func = self
            .prelude_copy(unit.file, name, types)?
            .expect("a function of the module");
        Ok(Some((func, shapes)))
    }

    /// The function `name` of the module of the prelude in `file`, which
    /// the lowering lowers at the first call. `None` when `file` is not a
    /// module of the prelude, or has no such function.
    pub(super) fn prelude_func(&self, file: File, name: &str) -> Result<Option<FuncId>, Error> {
        self.prelude_copy(file, name, Vec::new())
    }

    /// The copy of the function `name` of the module of the prelude in
    /// `file` whose type variables have the reps of `types`, as
    /// [`Self::prelude_func`] gives it.
    fn prelude_copy(
        &self,
        file: File,
        name: &str,
        types: Vec<(String, Rep)>,
    ) -> Result<Option<FuncId>, Error> {
        let Some(unit) = self.prelude.iter().find(|unit| unit.file == file) else {
            return Ok(None);
        };
        let Some((name, def)) = unit.defs.get_key_value(name) else {
            return Ok(None);
        };
        let reps = types.iter().map(|(_, rep)| *rep).collect();
        let (key, def) = ((file, *name, reps), *def);
        if let Some(func) = self.prelude_funcs.borrow().get(&key) {
            return Ok(Some(*func));
        }
        let outer = self.unit.replace(unit);
        let outer_types = self.types.replace(types.clone());
        let signature = self.signature(def);
        self.types.replace(outer_types);
        self.unit.set(outer);
        let (id, params, ret) = signature?;
        let sig = Sig {
            unit,
            def: Some(def),
            defaults: params.iter().map(|_| None).collect(),
            doctest: None,
            params,
            ret,
            types,
        };
        // Each copy of a generic function is a function of its own, which
        // no id of the checker names.
        let func = if def.type_params.is_some() {
            self.funcs.borrow_mut().add_generated(sig, true)
        } else {
            self.funcs.borrow_mut().add(id, sig)
        };
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
            "item" => arg().zip(arg()).and_then(|(list, index)| {
                let rep = self.list_item(list.rep().list()?);
                Some(Expr::Item { list, index, rep })
            }),
            "list_capacity" => arg().map(Expr::ListCapacity),
            "list_reserve" => arg().zip(arg()).map(|(list, capacity)| Expr::ListReserve {
                list,
                capacity,
                site: self.site(call.range()),
            }),
            "list_set_len" => arg()
                .zip(arg())
                .map(|(list, len)| Expr::ListSetLen { list, len }),
            // The value takes the rep of the items of the list.
            "set_item" => {
                let (list, index, value) = (arg(), arg(), arg());
                list.zip(index)
                    .zip(value)
                    .and_then(|((list, index), value)| {
                        let item = self.list_item(list.rep().list()?);
                        Some(Expr::SetItem {
                            list,
                            index,
                            value: Box::new(coerce(*value, item)?),
                        })
                    })
            }
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
/// prelude, whose declared type is `shape`, and `None` when the parameter
/// does not take it. A parameter of `int`, or of a union with `int`, takes
/// a `bool` as the `int` it is, because the prelude reads the union as an
/// `int` after a test. A parameter of a type variable takes it as an `int`
/// only when the variable is `int`, so a list of `bool`s keeps its items.
/// Any other parameter keeps the `bool`.
fn prelude_arg(value: Expr, rep: Rep, shape: &Shape) -> Option<Expr> {
    let as_int = match shape {
        Shape::TypeVar(_) => rep == Rep::Int,
        _ => matches!(rep, Rep::Int | Rep::Uniform),
    };
    match value.rep() {
        Rep::Bool if as_int => coerce(Expr::BoolAsInt(Box::new(value)), rep),
        _ => coerce(value, rep),
    }
}

/// The rep of each type variable of `names` for a call with arguments of
/// `args` of a function whose parameters have the declared types `params`.
/// `item` gives the rep of the items of a type of list. Only a variable
/// inside `list[...]` takes the rep of the argument, since a list of one
/// type holds no item of another, and a bare variable takes the argument
/// as a value of that rep. `None` when a variable gets no rep or two.
fn type_map(
    names: &[String],
    params: &[Shape],
    args: &[Rep],
    item: impl Fn(ListId) -> Rep,
) -> Option<Vec<(String, Rep)>> {
    let mut found: Vec<Option<Rep>> = names.iter().map(|_| None).collect();
    let mut bind = |name: &str, rep: Rep| -> Option<()> {
        let Some(slot) = names
            .iter()
            .position(|var| var == name)
            .and_then(|i| found.get_mut(i))
        else {
            return Some(());
        };
        match slot {
            Some(bound) if *bound != rep => None,
            _ => {
                *slot = Some(rep);
                Some(())
            }
        }
    };
    for (shape, rep) in params.iter().zip(args) {
        let (mut shape, mut rep) = (shape, *rep);
        // A list or `None` is a list for `list[T] | None`.
        if let (Shape::Union(shapes), Rep::ListOrNone(list)) = (shape, rep)
            && let Some(member) = shapes.iter().find(|shape| matches!(shape, Shape::List(_)))
        {
            (shape, rep) = (member, Rep::List(list));
        }
        // The item of a list, and of each list inside it.
        while let (Shape::List(inner), Rep::List(list)) = (shape, rep) {
            shape = inner;
            rep = item(list);
            if let Shape::TypeVar(name) = shape {
                bind(name, rep)?;
            }
        }
    }
    names
        .iter()
        .zip(found)
        .map(|(name, rep)| Some((name.clone(), rep?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use engine::Level;
    use engine::check::check_source;
    use ty_python_semantic::types::lowering::Shape;

    use crate::ir::{ListId, Rep};

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

    /// The reps that [`super::type_map`] gives to `T` for `params` and
    /// `args`, where the list 0 has items of `int`, the list 1 of `bool`,
    /// and the list 2 has lists 0 as its items.
    fn types_of(params: &[Shape], args: &[Rep]) -> Option<Vec<(String, Rep)>> {
        let items = [Rep::Int, Rep::Bool, Rep::List(ListId::new(0))];
        super::type_map(&["T".to_owned()], params, args, |list| {
            *items.get(list.index()).expect("a list of the test")
        })
    }

    fn list_of(shape: Shape) -> Shape {
        Shape::List(Box::new(shape))
    }

    #[test]
    fn a_type_variable_takes_the_rep_of_the_items_of_a_list() {
        let t = || Shape::TypeVar("T".to_owned());
        let list = |i| Rep::List(ListId::new(i));
        let int = Some(vec![("T".to_owned(), Rep::Int)]);
        assert_eq!(types_of(&[list_of(t()), t()], &[list(0), Rep::Bool]), int);
        assert_eq!(
            types_of(&[list_of(t())], &[list(1)]),
            Some(vec![("T".to_owned(), Rep::Bool)])
        );
        assert_eq!(types_of(&[list_of(list_of(t()))], &[list(2)]), int);
        assert_eq!(
            types_of(&[list_of(t()), list_of(t())], &[list(0), list(1)]),
            None
        );
        assert_eq!(types_of(&[t()], &[Rep::Int]), None);
        assert_eq!(types_of(&[Shape::Int], &[Rep::Int]), None);
    }

    #[test]
    fn a_program_has_each_function_of_the_prelude_that_it_reaches_once() {
        assert_eq!(funcs("print('oi')\n"), 0);
        assert_eq!(
            funcs("import math\n\nprint(math.isnan(1.0), math.isnan(2.0))\n"),
            1
        );
    }

    #[test]
    fn a_generic_function_of_the_prelude_has_a_copy_for_each_type_of_list() {
        // `append` and the `grow` that it calls.
        assert_eq!(funcs("xs = [1]\nxs.append(2)\nxs.append(3)\n"), 2);
        assert_eq!(
            funcs("xs = [1]\nys = ['a']\nxs.append(2)\nys.append('b')\n"),
            4
        );
    }
}
