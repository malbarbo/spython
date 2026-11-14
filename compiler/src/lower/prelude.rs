//! The functions of the prelude, which the lowering takes from the Python
//! of the package of the prelude when a call reaches them.

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{ImportingFile, ModuleName, resolve_module};
use ty_python_semantic::Db;
use ty_python_semantic::types::lowering::{DefId, Shape, call_bindings, expr_shape, function_info};

use std::rc::Rc;

use super::class::{generated_call, param};
use super::{Loaded, Lowering, PreludeParams, Sig, TypeArg, Unit, params_of, unsupported};
use crate::Error;
use crate::cpython::names;
use crate::ir::{
    Case, ClassId, CodePointSet, Expr, FuncId, FuncType, FuncTypeId, ListId, Rep, Site, Stmt,
};

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
        let name = attr.attr.as_str();
        // A method through its type, as `float.fromhex(s)` or
        // `str.upper(s)`, takes each argument in the place of a parameter.
        if let ast::Expr::Name(_) = &*attr.value
            && let Some(class) = self.builtin_name(&attr.value)
        {
            let unit = builtin_module(&class, name)
                .and_then(|module| self.prelude_unit(module))
                .ok_or_else(|| unsupported(call, "a call of this method"))?;
            return self.bound_call(call, (unit, name), None);
        }
        let receiver = self.expr(&attr.value)?;
        let list =
            matches!(receiver.rep(), Rep::Object(_)) && self.list_class(receiver.rep()).is_some();
        if !list
            && let Some(class) = receiver.rep().class()
            && let Some(file) = self.copy_file(class)
        {
            return self.copy_method_call(call, attr, receiver, (class, file));
        }
        if let Rep::Object(class) = receiver.rep()
            && let Some(method) = self.method(class, name)
        {
            return self.call_method(call, attr, receiver, method);
        }
        let class = match receiver.rep() {
            _ if list => "list",
            Rep::Str => "str",
            Rep::Int | Rep::Bool => "int",
            Rep::Float => "float",
            _ => "",
        };
        let module = builtin_module(class, name)
            .ok_or_else(|| unsupported(call, "a call of this method"))?;
        if module == "list" && name == "sort" {
            return self.list_sort(call, receiver);
        }
        let unit = self
            .prelude_unit(module)
            .ok_or_else(|| unsupported(call, "a call of this method"))?;
        self.bound_call(call, (unit, name), Some(receiver))
    }

    /// `attr`, a property of `expr`, a number of `rep`, as the call of the
    /// function of the same name in the module of its type in the prelude.
    /// The checker gives a method a type of its own, and a property the
    /// type of its value.
    pub(super) fn number_property(
        &self,
        expr: &ast::Expr,
        attr: &ast::ExprAttribute,
        rep: Rep,
    ) -> Result<Expr, Error> {
        let name = attr.attr.as_str();
        let (module, attributes) = match rep {
            Rep::Float => ("float", names::FLOAT_ATTRIBUTES),
            _ => ("int", names::INT_ATTRIBUTES),
        };
        let value = matches!(
            expr_shape(self.model(), expr),
            Some(shape) if !matches!(shape, Shape::BoundMethod(..) | Shape::Function(_))
        );
        if !value || name.starts_with('_') || !attributes.contains(&name) {
            return Err(unsupported(expr, "this attribute"));
        }
        let number = self.expr(&attr.value)?;
        self.prelude_call(expr, module, name, vec![number])
    }

    /// The call of the function `name` of `unit`, a module of the prelude,
    /// for `call`, a call of a builtin function or of a method on
    /// `receiver`, which takes the first parameter. The checker binds each
    /// argument to a parameter of the function in the stubs. A positional
    /// argument goes to the parameter in its place, and a keyword argument
    /// to the parameter of the same name in the prelude. A parameter
    /// without an argument takes its default.
    pub(super) fn bound_call(
        &self,
        call: &ast::ExprCall,
        (unit, name): (&'a Unit<'a, 'db>, &str),
        receiver: Option<Expr>,
    ) -> Result<Expr, Error> {
        let what = match receiver {
            Some(_) => "method",
            None => "function",
        };
        let def = *unit
            .defs
            .get(name)
            .ok_or_else(|| unsupported(call, &format!("a call of this {what}")))?;
        let params: Vec<&ast::ParameterWithDefault> = params_of(def).collect();
        let bindings = call_bindings(self.model(), call)
            .ok_or_else(|| unsupported(call, "a call with these arguments"))?;
        // `join` makes a list of a generator first, as `PySequence_Fast`
        // does.
        let join = unit.prelude == Some("str") && name == "join";
        let mut slots: Vec<Option<u32>> = params.iter().map(|_| None).collect();
        let first = usize::from(receiver.is_some());
        let mut args: Vec<Expr> = receiver.into_iter().collect();
        if let (1, Some(slot)) = (first, slots.first_mut()) {
            *slot = Some(0);
        }
        for (i, arg) in call.arguments.iter_source_order().enumerate() {
            // A positional parameter of the stubs may have another name in
            // the prelude, and a keyword argument has to find its name.
            let param = match arg {
                ast::ArgOrKeyword::Arg(_) => Some(i + first),
                ast::ArgOrKeyword::Keyword(_) => bindings
                    .get(i)
                    .and_then(|binding| binding.parameter_name.as_deref())
                    .and_then(|bound| {
                        params
                            .iter()
                            .skip(first)
                            .position(|param| param.name().as_str() == bound)
                    })
                    .map(|param| param + first),
            };
            let Some(slot) = param
                .and_then(|param| slots.get_mut(param))
                .filter(|slot| slot.is_none())
            else {
                return Err(unsupported(
                    call,
                    &format!("a call of this {what} with these arguments"),
                ));
            };
            *slot = Some(u32::try_from(args.len()).expect("fewer than 2^32 arguments"));
            args.push(match arg.value() {
                value @ ast::Expr::Generator(_) if join => self.consumed_list(call, value)?,
                value => self.expr(value)?,
            });
        }
        for (slot, param) in slots.iter_mut().zip(&params) {
            if slot.is_none() {
                let default = param.default.as_deref().and_then(literal).ok_or_else(|| {
                    unsupported(call, &format!("a call of this {what} with these arguments"))
                })?;
                *slot = Some(u32::try_from(args.len()).expect("fewer than 2^32 arguments"));
                args.push(default);
            }
        }
        let order: Vec<u32> = slots
            .into_iter()
            .map(|slot| slot.expect("an argument for each parameter"))
            .collect();
        let reps: Vec<Rep> = order
            .iter()
            .filter_map(|arg| args.get(*arg as usize))
            .map(Expr::rep)
            .collect();
        let (func, params) = self
            .prelude_for(call, unit, name, &reps)?
            .ok_or_else(|| unsupported(call, &format!("a call of this {what}")))?;
        let sig = self.funcs.borrow().get(func).clone();
        let mut param_of = vec![0; args.len()];
        for (param, arg) in order.iter().enumerate() {
            if let Some(slot) = param_of.get_mut(*arg as usize) {
                *slot = param;
            }
        }
        let args = args
            .into_iter()
            .zip(param_of)
            .map(|(value, param)| {
                sig.params
                    .get(param)
                    .zip(params.0.get(param))
                    .and_then(|(rep, shape)| self.prelude_arg(value, *rep, shape))
                    .ok_or_else(|| unsupported(call, "an argument of this type"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let in_place = order.iter().map(|arg| *arg as usize).eq(0..args.len());
        Ok(Expr::Call {
            func,
            args,
            order: (!in_place).then_some(order),
            rep: sig.ret,
            site: self.call_site(func, call),
            check: None,
        })
    }

    /// The call of the method of `attr` through `receiver`, an object of
    /// `class`, a copy of a generic class of the prelude in `file`. The call
    /// takes the method of the class directly, with the arguments as a
    /// call of a method of a builtin type takes them.
    fn copy_method_call(
        &self,
        call: &ast::ExprCall,
        attr: &ast::ExprAttribute,
        receiver: Expr,
        (class, file): (ClassId, File),
    ) -> Result<Expr, Error> {
        let name = attr.attr.as_str();
        let mut method = self
            .method(class, name)
            .ok_or_else(|| unsupported(call, "a call of this method"))?;
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(
                call,
                "a call of a method with keyword arguments",
            ));
        }
        // A call with one more argument goes to the method that takes a
        // default, as `get_or` for `get`, and a default of `None` to the
        // method without it, as `get_or_none`, since a type variable never
        // stands for `None`.
        let mut args = &call.arguments.args[..];
        if args.len() >= method.def.parameters.args.len() {
            let none = match args {
                [rest @ .., ast::Expr::NoneLiteral(_)] => self
                    .method(class, &format!("{name}_or_none"))
                    .map(|method| (rest, method)),
                _ => None,
            };
            if let Some((rest, with_none)) = none {
                (args, method) = (rest, with_none);
            } else if let Some(with_default) = self.method(class, &format!("{name}_or")) {
                method = with_default;
            }
        }
        let text = format!("'NoneType' object has no attribute '{name}'");
        let mut given = vec![self.not_none(receiver, text, attr)];
        for arg in args {
            given.push(self.expr(arg)?);
        }
        let func = match method.func {
            Some(func) => func,
            None => {
                let args = self.method_type_args(call, method.def)?;
                self.generic_copy_method(class, method.def, args)
            }
        };
        let unit = self.prelude_unit_of(file);
        let info = function_info(unit.model, method.def).expect("a method of the prelude");
        let shapes: Vec<Shape> = info.params.into_iter().map(|param| param.shape).collect();
        let args = self.prelude_args(
            call,
            (func, &shapes),
            given,
            (true, "a call of this method with these arguments"),
        )?;
        let site = self.call_site(func, call);
        Ok(self.prelude_call_of(func, args, site))
    }

    /// What each type variable of `def`, a generic method of a copy of a
    /// generic class of the prelude, stands for in `call`. A type variable
    /// stands for the type of the argument of the parameter that it
    /// annotates.
    fn method_type_args(
        &self,
        call: &ast::ExprCall,
        def: &ast::StmtFunctionDef,
    ) -> Result<Vec<TypeArg>, Error> {
        let params = def
            .type_params
            .iter()
            .flat_map(|params| &params.type_params);
        params
            .map(|param| {
                let name = param.name().as_str();
                let arg = def
                    .parameters
                    .args
                    .iter()
                    .skip(1)
                    .zip(&call.arguments.args)
                    .find_map(|(param, arg)| match param.parameter.annotation.as_deref() {
                        Some(ast::Expr::Name(annotation)) if annotation.id.as_str() == name => {
                            Some(arg)
                        }
                        _ => None,
                    })
                    .expect("a type variable of a method that a parameter takes");
                expr_shape(self.model(), arg)
                    .and_then(|shape| self.type_arg(shape))
                    .ok_or_else(|| unsupported(arg, "an argument of this type"))
            })
            .collect()
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

    /// The call at `node` of the function `name` of the module `module` of
    /// the prelude with `args`, in the copy for the reps of `args`.
    pub(super) fn module_call(
        &self,
        node: &impl Ranged,
        module: &str,
        name: &str,
        args: Vec<Expr>,
    ) -> Result<Expr, Error> {
        let unit = self.prelude_unit(module).expect("a module of the prelude");
        let (func, args) = self
            .generic_args(
                node,
                (unit, name),
                args,
                (false, "a call with these arguments"),
            )?
            .expect("a function of the module");
        let site = self.call_site(func, node);
        Ok(self.prelude_call_of(func, args, site))
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
            Rep::Object(_) => "repr_of",
            _ => "repr_or_none",
        };
        self.list_call_at(&TextRange::default(), name, vec![value], site)
    }

    /// Lowers the function of the module `list` of the prelude that gives
    /// the `repr` of a list of `rep`, which the `str` of the class `List`
    /// calls when the program writes the list. A list of a value without a
    /// `repr` fails here, at the write, and not in the code of the class.
    pub(super) fn list_repr_func(&self, rep: Rep) -> Result<(), Error> {
        let unit = self
            .prelude_unit("list")
            .expect("the module list of the prelude");
        let object = self.list_class(rep).map(Rep::Object).expect("a list");
        self.prelude_for(&TextRange::default(), unit, "repr_of", &[object])?
            .expect("the repr of a list");
        Ok(())
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

    /// `other`, a value of any rep, as an object of `class` when its class
    /// is exactly `class`, and `None` otherwise, as an object or `None`.
    fn exact_of(&self, class: ClassId, other: Expr) -> Expr {
        let rep = Rep::ObjectOrNone(class);
        let other = self.coerce(other, Rep::Uniform).expect("a value as object");
        let found = Expr::AsExact {
            value: Box::new(param(0, Rep::Uniform)),
            class,
        };
        let body = vec![
            Stmt::If {
                cond: Expr::IsExact {
                    value: Box::new(param(0, Rep::Uniform)),
                    class,
                },
                then: vec![Stmt::Return(Some(
                    self.coerce(found, rep)
                        .expect("an object as an object or None"),
                ))],
                else_: Vec::new(),
            },
            Stmt::Return(Some(
                self.coerce(Expr::None, rep)
                    .expect("None as an object or None"),
            )),
        ];
        let func = self.generated(vec![Rep::Uniform], rep, body);
        generated_call(func, vec![other], rep, Site::NONE)
    }

    /// The module of the prelude in `file`.
    pub(super) fn prelude_unit_of(&self, file: File) -> &'a Unit<'a, 'db> {
        self.prelude
            .iter()
            .find(|unit| unit.file == file)
            .expect("a module of the prelude")
    }

    /// The module `module` of the prelude.
    pub(super) fn prelude_unit(&self, module: &str) -> Option<&'a Unit<'a, 'db>> {
        self.prelude
            .iter()
            .find(|unit| unit.prelude == Some(module))
    }

    /// The module `builtins` of the prelude, when it defines the builtin
    /// function `name`.
    pub(super) fn prelude_builtin(&self, name: &str) -> Option<&'a Unit<'a, 'db>> {
        self.prelude_unit("builtins")
            .filter(|unit| unit.defs.contains_key(name))
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
        let types = type_map(
            names,
            shapes,
            args,
            |list| self.list_arg(list),
            |ty| self.func_type_of(ty),
            |class| self.copy_args(class),
        )
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
        let (id, params, ret) = self.mark_prelude_error(signature)?;
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
        self.lowered
            .borrow_mut()
            .insert(func.index(), self.mark_prelude_error(lowered)?);
        Ok(Some(func))
    }

    /// `result`, the lowering of code of the prelude that the program asked
    /// for. An error in it is marked for [`Self::at_expression`].
    fn mark_prelude_error<T>(&self, mut result: Result<T, Error>) -> Result<T, Error> {
        if let Err(Error::Unsupported(error)) = &mut result
            && !self.in_prelude()
        {
            error.in_prelude = true;
        }
        result
    }

    /// `result`, the lowering of `expr`, with an error of the prelude that
    /// [`Self::mark_prelude_error`] marked at the range of `expr`, which is in
    /// the file of the program.
    pub(super) fn at_expression<T>(
        &self,
        expr: &ast::Expr,
        mut result: Result<T, Error>,
    ) -> Result<T, Error> {
        if let Err(Error::Unsupported(error)) = &mut result
            && error.in_prelude
            && !self.in_prelude()
        {
            error.in_prelude = false;
            error.range = expr.range();
        }
        result
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
            "mul_mod" => {
                let (a, b, m) = (arg(), arg(), arg());
                a.zip(b).zip(m).map(|((a, b), m)| Expr::MulMod { a, b, m })
            }
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
            "hash_of" => {
                let arg = arg().expect("hash_of has an argument");
                return self.hash_value(call, *arg, Site::NONE);
            }
            "exact" => {
                let class = arg().and_then(|x| x.rep().class());
                return Ok(self.exact_of(
                    class.expect("exact of an object"),
                    *arg().expect("exact has two arguments"),
                ));
            }
            "in_repr" => arg().map(Expr::InRepr),
            "during_repr" => arg()
                .zip(arg())
                .map(|(object, value)| Expr::DuringRepr { object, value }),
            "hash_slot" => arg()
                .zip(arg())
                .map(|(hash, mask)| Expr::HashSlot { hash, mask }),
            "hash_shift" => arg().map(Expr::HashShift),
            "hash_eq" => arg().zip(arg()).map(|(a, b)| Expr::HashEq(a, b)),
            "stop" => arg().map(|text| Expr::Stop {
                text,
                site: self.site(call.range()),
            }),
            "array_get" => arg().zip(arg()).and_then(|(list, index)| {
                let rep = self.list_item(list.rep().array()?);
                Some(Expr::Item { list, index, rep })
            }),
            "array_new" => {
                let annotated = self.annotation.get();
                let Some((_, Rep::Array(list))) =
                    annotated.filter(|(range, _)| *range == call.range())
                else {
                    return Err(unsupported(call, "an array without an annotation"));
                };
                arg().map(|len| Expr::ArrayNew {
                    list,
                    len,
                    site: self.site(call.range()),
                })
            }
            "array_clear" => {
                let (array, start, count) = (arg(), arg(), arg());
                array
                    .zip(start)
                    .zip(count)
                    .map(|((array, start), count)| Expr::ArrayClear {
                        array,
                        start,
                        count,
                    })
            }
            // A value of a rep without a reference stands for an object
            // that the program can only compare with `==`.
            "same" => arg().zip(arg()).map(|(a, b)| match a.rep() {
                Rep::Object(_) | Rep::ObjectOrNone(_) | Rep::Array(_) | Rep::Uniform => {
                    Expr::Is(a, b)
                }
                _ => Expr::Bool(false),
            }),
            "array_len" => arg().map(Expr::ArrayLen),
            // The value takes the rep of the items of the array.
            "array_set" => {
                let (list, index, value) = (arg(), arg(), arg());
                list.zip(index)
                    .zip(value)
                    .and_then(|((list, index), value)| {
                        let item = self.list_item(list.rep().array()?);
                        Some(Expr::SetItem {
                            list,
                            index,
                            value: Box::new(self.coerce(*value, item)?),
                        })
                    })
            }
            "array_copy" => {
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
/// `item` gives the rep of the items of a type of list, and `func` the
/// parameters and the result of a type of function. A variable inside
/// `list[...]`, `Array[...]` or `Callable[...]` takes the rep of the
/// argument, since a list of one type holds no item of another, and a bare variable takes
/// the argument as a value of that rep. A variable that only bare
/// parameters have takes the rep of their argument, unless it is uniform,
/// whose classes the call does not know, or `None`, which no parameter
/// holds. `None` when a variable gets no rep or two.
fn type_map(
    names: &[String],
    params: &[Shape],
    args: &[Rep],
    item: impl Fn(ListId) -> TypeArg,
    func: impl Fn(FuncTypeId) -> FuncType,
    copy: impl Fn(ClassId) -> Option<Vec<TypeArg>>,
) -> Option<Vec<(String, TypeArg)>> {
    let mut found: Vec<Option<TypeArg>> = names.iter().map(|_| None).collect();
    // A type of function gives a variable its rep alone, so a variable
    // that a list binds keeps the classes that the list gives it.
    let mut bind = |name: &str, arg: TypeArg, rep_alone: bool| -> Option<()> {
        let Some(slot) = names
            .iter()
            .position(|var| var == name)
            .and_then(|i| found.get_mut(i))
        else {
            return Some(());
        };
        match slot {
            Some(bound) if rep_alone && bound.rep == arg.rep => Some(()),
            Some(bound) if *bound != arg => None,
            _ => {
                *slot = Some(arg);
                Some(())
            }
        }
    };
    for (shape, rep) in params.iter().zip(args) {
        let (mut shape, mut rep) = (shape, *rep);
        // An object or `None` is an object for `List[T] | None`.
        if let (Shape::Union(shapes), Rep::ObjectOrNone(class)) = (shape, rep)
            && let [member] = &shapes
                .iter()
                .filter(|shape| !matches!(shape, Shape::None))
                .collect::<Vec<_>>()[..]
        {
            (shape, rep) = (member, Rep::Object(class));
        }
        // A parameter and the result of a function take the reps of the
        // type of the function.
        if let (Shape::Callable(shapes, ret), Rep::Func(ty)) = (shape, rep) {
            let FuncType {
                params,
                ret: ret_rep,
            } = func(ty);
            for (shape, rep) in shapes.iter().zip(params).chain([(&**ret, ret_rep)]) {
                if let Shape::TypeVar(name) = shape {
                    let classes = Vec::new();
                    bind(name, TypeArg { rep, classes }, true)?;
                }
            }
            continue;
        }
        // The item of a list or of an array, what each type variable of a
        // copy of a class of the prelude stands for, and the same inside
        // each of them.
        let mut inner = vec![(shape, rep)];
        while let Some((shape, rep)) = inner.pop() {
            for (shape, arg) in type_args(shape, rep, &item, &copy) {
                if let Shape::TypeVar(name) = shape {
                    bind(name, arg.clone(), false)?;
                }
                inner.push((shape, arg.rep));
            }
        }
    }
    let bound: Vec<bool> = found.iter().map(Option::is_some).collect();
    for (shape, rep) in params.iter().zip(args) {
        let Shape::TypeVar(name) = shape else {
            continue;
        };
        let Some(i) = names.iter().position(|var| var == name) else {
            continue;
        };
        if bound.get(i) == Some(&true) {
            continue;
        }
        match found.get_mut(i) {
            Some(slot @ None) if !matches!(rep, Rep::Uniform | Rep::None) => {
                *slot = Some(TypeArg {
                    rep: *rep,
                    classes: Vec::new(),
                });
            }
            Some(Some(arg)) if arg.rep == *rep => {}
            _ => return None,
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

/// The value of `default`, the default of a parameter of the prelude, which
/// is a literal so that the call can give it in place of the `def`.
fn literal(default: &ast::Expr) -> Option<Expr> {
    match default {
        ast::Expr::NoneLiteral(_) => Some(Expr::None),
        ast::Expr::BooleanLiteral(value) => Some(Expr::Bool(value.value)),
        ast::Expr::StringLiteral(value) => Some(Expr::Str(value.value.to_str().to_owned())),
        ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: ast::Number::Int(value),
            ..
        }) => value.as_i32().map(Expr::Int),
        ast::Expr::UnaryOp(ast::ExprUnaryOp {
            op: ast::UnaryOp::USub,
            operand,
            ..
        }) => match literal(operand)? {
            Expr::Int(value) => Some(Expr::Int(-value)),
            _ => None,
        },
        _ => None,
    }
}

/// The module of the prelude with the method `name` of the builtin type
/// `class`. The module `str` of the prelude defines each method of `str`
/// that it has under the name of the method, among other functions, and
/// the modules `list`, `int` and `float` each method of their type. A
/// `bool` takes the methods of `int`.
fn builtin_module(class: &str, name: &str) -> Option<&'static str> {
    let public = |attributes: &[&str]| !name.starts_with('_') && attributes.contains(&name);
    match class {
        "str" if public(names::STR_ATTRIBUTES) => Some("str"),
        "list" if names::LIST_METHODS.contains(&name) => Some("list"),
        "int" | "bool" if public(names::INT_ATTRIBUTES) => Some("int"),
        "float" if public(names::FLOAT_ATTRIBUTES) => Some("float"),
        _ => None,
    }
}

/// Each type argument of a parameter of `shape`, with what it stands for
/// in an argument of `rep`. The items of an array take the type of the
/// items of the argument, and a list or a copy of a generic class of the
/// prelude, which `copy` gives, takes what its type variables stand for.
fn type_args(
    shape: &Shape,
    rep: Rep,
    item: impl Fn(ListId) -> TypeArg,
    copy: impl Fn(ClassId) -> Option<Vec<TypeArg>>,
) -> Vec<(&Shape, TypeArg)> {
    match (shape, rep) {
        (Shape::List(inner), Rep::Object(class) | Rep::ObjectOrNone(class)) => {
            match copy(class).as_deref() {
                Some([arg]) => vec![(&**inner, arg.clone())],
                _ => Vec::new(),
            }
        }
        (Shape::Instance(_, args), Rep::Array(list)) if args.len() == 1 => {
            args.iter().map(|inner| (inner, item(list))).collect()
        }
        (Shape::Instance(_, shapes), Rep::Object(class) | Rep::ObjectOrNone(class)) => {
            match copy(class) {
                Some(args) if args.len() == shapes.len() => shapes.iter().zip(args).collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use engine::Level;
    use engine::check::check_source;
    use indoc::indoc;
    use ty_python_semantic::types::lowering::Shape;

    use crate::ir::{ClassId, Rep};
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

    /// The number of classes in the program of `source`.
    fn classes(source: &str) -> usize {
        let Ok(checked) = check_source(source, Level::Full, false) else {
            panic!("the checker rejects the program");
        };
        match crate::lower::lower(&checked.db, checked.file, "test.py", None) {
            Ok(program) => program.classes.len(),
            Err(error) => panic!("{error:?}"),
        }
    }

    /// The reps that [`super::type_map`] gives to `T` for `params` and
    /// `args`, where the list of the class 0 has items of `int`, the list
    /// of the class 1 of `bool`, and the list of the class 2 has lists of
    /// the class 0 as its items.
    fn types_of(params: &[Shape], args: &[Rep]) -> Option<Vec<(String, Rep)>> {
        let items = [Rep::Int, Rep::Bool, list(0)];
        let types = super::type_map(
            &["T".to_owned()],
            params,
            args,
            |_| unreachable!("no array in the test"),
            |_| unreachable!("no function in the test"),
            |class| {
                let rep = *items.get(class.index())?;
                Some(vec![TypeArg {
                    rep,
                    classes: Vec::new(),
                }])
            },
        )?;
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

    /// An object of the class `i` of [`types_of`], a list.
    fn list(i: usize) -> Rep {
        Rep::Object(ClassId::new(i))
    }

    #[test]
    fn a_type_variable_takes_the_rep_of_the_items_of_a_list() {
        let t = || Shape::TypeVar("T".to_owned());
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
        assert_eq!(types_of(&[Shape::Int], &[Rep::Int]), None);
    }

    #[test]
    fn a_type_variable_that_no_list_has_takes_the_rep_of_its_argument() {
        let t = || Shape::TypeVar("T".to_owned());
        let int = Some(vec![("T".to_owned(), Rep::Int)]);
        assert_eq!(types_of(&[t()], &[Rep::Int]), int);
        assert_eq!(types_of(&[t(), t()], &[Rep::Int, Rep::Int]), int);
        assert_eq!(types_of(&[t(), t()], &[Rep::Int, Rep::Str]), None);
        assert_eq!(types_of(&[t()], &[Rep::Uniform]), None);
        assert_eq!(types_of(&[t()], &[Rep::None]), None);
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
        let one = funcs("xs = [1]\nxs.append(2)\n");
        assert_eq!(funcs("xs = [1]\nxs.append(2)\nxs.append(3)\n"), one);
        assert_eq!(
            funcs("xs = [1]\nys = [2]\nxs.append(2)\nys.append(3)\n"),
            one
        );
        assert!(funcs("xs = [1]\nys = ['a']\nxs.append(2)\nys.append('b')\n") > one);
    }

    #[test]
    fn a_generic_class_of_the_prelude_has_a_copy_for_each_type() {
        assert_eq!(
            classes(indoc! {"
                def f(a: dict[str, int], b: dict[str, int], c: dict[str, str], d: set[int]) -> None:
                    pass
            "}),
            3
        );
        assert_eq!(
            classes(indoc! {"
                def f(a: dict[str, dict[str, int]], b: dict[str, int]) -> None:
                    pass
            "}),
            2
        );
    }

    #[test]
    fn a_method_of_a_copy_comes_with_its_first_call() {
        let len = |params: &str, sum: &str| {
            funcs(&format!("def f({params}) -> int:\n    return {sum}\n"))
        };
        let one = len("a: dict[str, int]", "len(a)");
        assert_eq!(len("a: dict[str, int]", "0"), 1);
        assert_eq!(
            len("a: dict[str, int], b: dict[str, int]", "len(a) + len(b)"),
            one
        );
        assert!(len("a: dict[str, int], b: dict[int, int]", "len(a) + len(b)") > one);
    }

    #[test]
    fn a_call_of_a_method_of_a_dict_calls_the_method_of_the_copy() {
        assert_eq!(
            funcs(indoc! {"
                def f(d: dict[str, int], e: dict[str, int]) -> None:
                    d.clear()
                    e.clear()
            "}),
            funcs(indoc! {"
                def f(d: dict[str, int], e: dict[str, int]) -> None:
                    d.clear()
            "}),
        );
    }
}

#[cfg(test)]
mod stubs {
    use engine::Level;
    use engine::check::check_source;
    use ruff_db::Db as _;
    use ruff_python_ast as ast;

    use crate::cpython::names;

    /// The minor version of Python 3 that the checker and the tests take.
    const MINOR: u32 = 13;

    /// Each method of `class` in the stub of the builtins, or each function
    /// of the module with `None`, with the names of the parameters after
    /// `self` that a keyword argument can name, by place. A parameter that
    /// takes only a position has no name.
    fn stub_functions(class: Option<&str>) -> Vec<(String, Vec<Option<String>>)> {
        let Ok(checked) = check_source("", Level::Full, false) else {
            panic!("the checker rejects an empty program");
        };
        let stub = checked
            .db
            .vendored()
            .read_to_string("stdlib/builtins.pyi")
            .expect("the stub of the builtins");
        let module = ruff_python_parser::parse_module(&stub)
            .expect("a stub that parses")
            .into_syntax();
        let body = match class {
            Some(class) => module
                .body
                .iter()
                .find_map(|stmt| match stmt {
                    ast::Stmt::ClassDef(def) if def.name.as_str() == class => Some(&def.body),
                    _ => None,
                })
                .expect("the class in the stub"),
            None => &module.body,
        };
        let mut functions = Vec::new();
        collect(body, usize::from(class.is_some()), &mut functions);
        functions
    }

    /// The functions in `body`, without the first `skip` parameters, with
    /// the branch of each `if` on `sys.version_info` that [`MINOR`] takes.
    fn collect(
        body: &[ast::Stmt],
        skip: usize,
        functions: &mut Vec<(String, Vec<Option<String>>)>,
    ) {
        for stmt in body {
            match stmt {
                ast::Stmt::FunctionDef(def) => {
                    let parameters = &def.parameters;
                    let positional = parameters
                        .posonlyargs
                        .iter()
                        .map(|_| None)
                        .chain(
                            parameters
                                .args
                                .iter()
                                .map(|param| Some(param.name().to_string())),
                        )
                        .skip(skip);
                    functions.push((def.name.to_string(), positional.collect()));
                }
                ast::Stmt::If(stmt) => {
                    let taken = version_test(&stmt.test).is_none_or(|minor| minor <= MINOR);
                    if taken {
                        collect(&stmt.body, skip, functions);
                    } else if let Some(clause) = stmt.elif_else_clauses.first() {
                        collect(&clause.body, skip, functions);
                    }
                }
                _ => {}
            }
        }
    }

    /// The minor version `N` of a test `sys.version_info >= (3, N)`.
    fn version_test(test: &ast::Expr) -> Option<u32> {
        let ast::Expr::Compare(compare) = test else {
            return None;
        };
        let [ast::Expr::Tuple(version)] = compare.comparators() else {
            return None;
        };
        match version.elts.get(1)? {
            ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
                value: ast::Number::Int(minor),
                ..
            }) => minor.as_u64().and_then(|minor| u32::try_from(minor).ok()),
            _ => None,
        }
    }

    /// The parameters after the first `skip` of each function of the module
    /// of the prelude in `file`, by name.
    fn prelude_params(file: &str, skip: usize) -> Vec<(String, Vec<String>)> {
        let (_, source) = ::prelude::FILES
            .iter()
            .find(|(path, _)| *path == file)
            .expect("the module in the prelude");
        ruff_python_parser::parse_module(source)
            .expect("a prelude that parses")
            .into_syntax()
            .body
            .iter()
            .filter_map(|stmt| match stmt {
                ast::Stmt::FunctionDef(def) => Some((
                    def.name.to_string(),
                    def.parameters
                        .iter_non_variadic_params()
                        .skip(skip)
                        .map(|param| param.name().to_string())
                        .collect(),
                )),
                _ => None,
            })
            .collect()
    }

    /// The methods of `class`, or the functions of the module with `None`,
    /// whose prelude function in `file` names a parameter otherwise than
    /// the stub, where a keyword argument can name it.
    fn mismatches(class: Option<&str>, file: &str, methods: &[&str]) -> Vec<String> {
        let prelude = prelude_params(file, usize::from(class.is_some()));
        let mut wrong = Vec::new();
        for (name, stub) in stub_functions(class) {
            if !methods.contains(&name.as_str()) {
                continue;
            }
            let Some((_, ours)) = prelude.iter().find(|(def, _)| *def == name) else {
                continue;
            };
            for (place, param) in stub.iter().enumerate() {
                if let Some(param) = param
                    && ours.get(place) != Some(param)
                {
                    wrong.push(format!(
                        "{name}: {param} at {place}, not {:?}",
                        ours.get(place)
                    ));
                }
            }
        }
        wrong
    }

    #[test]
    fn a_method_of_str_has_the_parameters_of_the_stub() {
        let wrong = mismatches(Some("str"), "str.py", names::STR_ATTRIBUTES);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn a_method_of_list_has_the_parameters_of_the_stub() {
        let wrong = mismatches(Some("list"), "list.py", names::LIST_METHODS);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn a_method_of_int_and_float_has_the_parameters_of_the_stub() {
        let mut wrong = mismatches(Some("int"), "int.py", names::INT_ATTRIBUTES);
        wrong.extend(mismatches(
            Some("float"),
            "float.py",
            names::FLOAT_ATTRIBUTES,
        ));
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn a_builtin_function_has_the_parameters_of_the_stub() {
        let wrong = mismatches(None, "builtins.py", names::BUILTINS);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}
