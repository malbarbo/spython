//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{DefId, Shape, function_info};

use std::rc::Rc;

use super::{Loaded, Lowering, PreludeParams, Sig, TypeArg, Unit, unsupported};
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
        if let ast::Expr::Call(sup) = &*attr.value
            && self.builtin_name(&sup.func).as_deref() == Some("super")
        {
            return self.super_call(call, attr, sup);
        }
        let receiver = self.expr(&attr.value)?;
        let name = attr.attr.as_str();
        if let Rep::Object(class) = receiver.rep()
            && let Some(method) = self.method(class, name)
        {
            return self.call_method(call, attr, receiver, method);
        }
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
        let (func, args) = match self.prelude_unit(module) {
            Some(unit) => self.generic_args(
                call,
                (unit, name),
                given,
                (true, "a call of this method with these arguments"),
            )?,
            None => None,
        }
        .ok_or_else(|| unsupported(call, "a call of this method"))?;
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
        let unit = self.prelude_unit_of(def.file);
        let (func, args) = self
            .generic_args(
                call,
                (unit, &def.name),
                given,
                (false, "a call with these arguments"),
            )?
            .expect("a function of the prelude");
        let site = self.call_site(func, call);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// The call at `node` of the function `name` of the module `list` of
    /// the prelude with `args`, in the copy for the reps of `args`.
    pub(super) fn list_call(
        &self,
        node: &impl Ranged,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<Expr, Error> {
        let (func, args) = self.list_args(node, name, args)?;
        let site = self.call_site(func, node);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// [`Self::list_call`] with the call at `site`.
    pub(super) fn list_call_at(
        &self,
        node: &impl Ranged,
        name: &str,
        args: Vec<Expr>,
        site: Site,
    ) -> Result<Expr, Error> {
        let (func, args) = self.list_args(node, name, args)?;
        let raises = self.funcs.borrow().raises.get(func.index()).copied();
        Ok(self.prelude_call_of(func, args, raises.unwrap_or(true).then_some(site)))
    }

    /// The function `name` of the module `list` of the prelude, in the copy
    /// for the reps of `given`, with `given` as its arguments.
    fn list_args(
        &self,
        node: &impl Ranged,
        name: &str,
        given: Vec<Expr>,
    ) -> Result<(FuncId, Vec<Expr>), Error> {
        let unit = self
            .prelude_unit("list")
            .expect("the module list of the prelude");
        let found = self.generic_args(
            node,
            (unit, name),
            given,
            (false, "a call with these arguments"),
        )?;
        Ok(found.expect("a function of the module list"))
    }

    /// The function `name` of `unit`, a module of the prelude, in the copy
    /// for the reps of `given`, with `given` as its arguments, as
    /// [`Self::prelude_args`] converts them for the call at `node`. `None`
    /// when the module has no such function.
    fn generic_args(
        &self,
        node: &impl Ranged,
        (unit, name): (&'a Unit<'a, 'db>, &str),
        given: Vec<Expr>,
        (defaults, count): (bool, &str),
    ) -> Result<Option<(FuncId, Vec<Expr>)>, Error> {
        let reps: Vec<Rep> = given.iter().map(Expr::rep).collect();
        let Some((func, params)) = self.prelude_for(node, unit, name, &reps)? else {
            return Ok(None);
        };
        let args = self.prelude_args(node, (func, &params.0), given, (defaults, count))?;
        Ok(Some((func, args)))
    }

    /// `given` as the arguments of `func`, a function of the prelude whose
    /// parameters have the declared types `shapes`, for the call at `node`.
    /// When `defaults` holds, a missing argument is `None`, which stands
    /// for the default of the parameter. A number of arguments that does
    /// not fit is `count`, a construct that the compiler does not support.
    fn prelude_args(
        &self,
        node: &impl Ranged,
        (func, shapes): (FuncId, &[Shape]),
        given: Vec<Expr>,
        (defaults, count): (bool, &str),
    ) -> Result<Vec<Expr>, Error> {
        let params = self.funcs.borrow().get(func).params.clone();
        if given.len() > params.len() || (!defaults && given.len() < params.len()) {
            return Err(unsupported(node, count));
        }
        let mut given = given.into_iter();
        params
            .iter()
            .zip(shapes)
            .map(|(rep, shape)| {
                self.prelude_arg(given.next().unwrap_or(Expr::None), *rep, shape)
                    .ok_or_else(|| unsupported(node, "an argument of this type"))
            })
            .collect()
    }

    /// The `repr` of `value`, a list or a list or `None`, at `site`, which
    /// the module `list` of the prelude gives.
    pub(super) fn list_repr(&self, value: Expr, site: Site) -> Result<Expr, Error> {
        let name = match value.rep() {
            Rep::List(_) => "repr_of",
            _ => "repr_or_none",
        };
        self.list_call_at(&TextRange::default(), name, vec![value], site)
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

    /// The module of the prelude in `file`.
    fn prelude_unit_of(&self, file: File) -> &'a Unit<'a, 'db> {
        self.prelude
            .iter()
            .find(|unit| unit.file == file)
            .expect("a module of the prelude")
    }

    /// The module `module` of the prelude.
    fn prelude_unit(&self, module: &str) -> Option<&'a Unit<'a, 'db>> {
        self.prelude
            .iter()
            .find(|unit| unit.prelude == Some(module))
    }

    /// The function `name` of the module `module` of the prelude.
    pub(super) fn prelude_named(&self, module: &str, name: &str) -> Result<Option<FuncId>, Error> {
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
    /// parameter and the names of its type variables. A generic function gives the copy for the reps that the
    /// arguments give to its type variables. `None` when the module has no
    /// such function.
    fn prelude_for(
        &self,
        node: &impl Ranged,
        unit: &'a Unit<'a, 'db>,
        name: &str,
        args: &[Rep],
    ) -> Result<Option<(FuncId, Rc<PreludeParams>)>, Error> {
        let Some((name, def)) = unit.defs.get_key_value(name) else {
            return Ok(None);
        };
        let key = (unit.file, *name);
        let cached = self.prelude_params.borrow().get(&key).cloned();
        let params = match cached {
            Some(params) => params,
            None => {
                let info =
                    function_info(unit.model, def).ok_or_else(|| unsupported(node, "this call"))?;
                let shapes: Vec<Shape> = info.params.into_iter().map(|param| param.shape).collect();
                let names: Vec<String> = def
                    .type_params
                    .iter()
                    .flat_map(|params| &params.type_params)
                    .map(|param| param.name().to_string())
                    .collect();
                let params = Rc::new((shapes, names));
                self.prelude_params.borrow_mut().insert(key, params.clone());
                params
            }
        };
        let (shapes, names) = &*params;
        let types = type_map(names, shapes, args, |list| self.list_arg(list))
            .ok_or_else(|| unsupported(node, "a call of a generic function with these types"))?;
        let func = self
            .prelude_copy(unit.file, name, types)?
            .expect("a function of the module");
        Ok(Some((func, params)))
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
        types: Vec<(String, TypeArg)>,
    ) -> Result<Option<FuncId>, Error> {
        let Some(unit) = self.prelude.iter().find(|unit| unit.file == file) else {
            return Ok(None);
        };
        let Some((name, def)) = unit.defs.get_key_value(name) else {
            return Ok(None);
        };
        let reps = types.iter().map(|(_, arg)| arg.clone()).collect();
        let (key, def) = ((file, *name, reps), *def);
        if let Some(func) = self.prelude_funcs.borrow().get(&key) {
            return Ok(Some(*func));
        }
        let outer = self.unit.replace(unit);
        let outer_types = self.types.replace(types.clone());
        let signature = self.signature(def, None);
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
            "list_truncate" => arg()
                .zip(arg())
                .map(|(list, len)| Expr::ListTruncate { list, len }),
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
                            value: Box::new(self.coerce(*value, item)?),
                        })
                    })
            }
            "copy_items" => {
                let (dst, at, src, start, count) = (arg(), arg(), arg(), arg(), arg());
                dst.zip(at).zip(src.zip(start)).zip(count).map(
                    |(((dst, at), (src, start)), count)| Expr::CopyItems {
                        dst,
                        at,
                        src,
                        start,
                        count,
                    },
                )
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
    item: impl Fn(ListId) -> TypeArg,
) -> Option<Vec<(String, TypeArg)>> {
    let mut found: Vec<Option<TypeArg>> = names.iter().map(|_| None).collect();
    let mut bind = |name: &str, arg: TypeArg| -> Option<()> {
        let Some(slot) = names
            .iter()
            .position(|var| var == name)
            .and_then(|i| found.get_mut(i))
        else {
            return Some(());
        };
        match slot {
            Some(bound) if *bound != arg => None,
            _ => {
                *slot = Some(arg);
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
            let arg = item(list);
            rep = arg.rep;
            if let Shape::TypeVar(name) = shape {
                bind(name, arg)?;
            }
        }
    }
    names
        .iter()
        .zip(found)
        .map(|(name, arg)| Some((name.clone(), arg?)))
        .collect()
}

impl Lowering<'_, '_> {
    /// `value` as an argument of a parameter of `rep` of a function of the
    /// prelude, whose declared type is `shape`, and `None` when the parameter
    /// does not take it. A parameter of `int`, or of a union with `int`, takes
    /// a `bool` as the `int` it is, because the prelude reads the union as an
    /// `int` after a test. A parameter of a type variable takes it as an `int`
    /// only when the variable is `int`, so a list of `bool`s keeps its items.
    /// Any other parameter keeps the `bool`.
    fn prelude_arg(&self, value: Expr, rep: Rep, shape: &Shape) -> Option<Expr> {
        let as_int = match shape {
            Shape::TypeVar(_) => rep == Rep::Int,
            _ => matches!(rep, Rep::Int | Rep::Uniform),
        };
        match value.rep() {
            Rep::Bool if as_int => self.coerce(Expr::BoolAsInt(Box::new(value)), rep),
            _ => self.coerce(value, rep),
        }
    }
}

#[cfg(test)]
mod tests {
    use engine::Level;
    use engine::check::check_source;
    use ty_python_semantic::types::lowering::Shape;

    use crate::ir::{ListId, Rep};
    use crate::lower::TypeArg;

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
        let types = super::type_map(&["T".to_owned()], params, args, |list| TypeArg {
            rep: *items.get(list.index()).expect("a list of the test"),
            classes: Vec::new(),
        })?;
        Some(
            types
                .into_iter()
                .map(|(name, arg)| (name, arg.rep))
                .collect(),
        )
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
