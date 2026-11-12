//! From the syntax tree and the types of the checker to the IR.

mod call;
mod class;
mod doctest;
mod expr;
mod flow;
mod init;
mod math;
mod name_error;
mod prelude;
mod special;
mod stmt;
mod vars;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ruff_db::files::File;
use ruff_db::parsed::{ParsedModuleRef, parsed_module};
use ruff_db::source::{SourceText, source_text};
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange, TextSize};
use ty_python_semantic::types::lowering::{
    DefId, Shape, declared_shape, expr_shape, function_info, slot_shape,
};
use ty_python_semantic::{Db, SemanticModel};

use crate::cpython::traceback::{self, FileLines};
use crate::ir::{
    ClassId, Expr, Func, FuncId, Import, ImportId, ListId, ListType, Place, Program, Rep, Site,
    SiteFrame, Stmt, Var, VarId,
};
use crate::{Docstring, Error};
use class::Classes;
use doctest::{DoctestScope, Tests};
use flow::{Assigned, Bindings, always_returns};
use name_error::unbound_local;

/// The program of `file`. With `tests`, the docstrings of the file and
/// whether the report shows every example, the program runs the doctests
/// after the body of the file.
pub(crate) fn lower(
    db: &dyn Db,
    file: File,
    path: &str,
    tests: Option<(&[Docstring], &[String], bool)>,
) -> Result<Program, Error> {
    let main = Loaded::new(db, file);
    let prelude = prelude::load(db, file);
    let unit = Unit::new(&main, path, None);
    let prelude: Vec<Unit> = prelude
        .iter()
        .map(|(name, loaded)| Unit::new(loaded, name, Some(name)))
        .collect();
    let mut vars = Vars::default();
    let tests = tests.map(|(docstrings, copies, verbose)| Tests {
        docstrings,
        copies,
        verbose,
        failed: vars.add_unnamed(Rep::Int),
        errors: vars.add_unnamed(Rep::Int),
        outcome: vars.add_unnamed(Rep::Int),
    });
    let lowering = Lowering {
        unit: Cell::new(&unit),
        tests,
        prelude: &prelude,
        intrinsics: prelude::intrinsics(db, file),
        sites: RefCell::new(Vec::new()),
        vars: RefCell::new(vars),
        funcs: RefCell::new(Funcs::default()),
        imports: RefCell::new(Imports::default()),
        classes: RefCell::new(Classes::default()),
        lists: RefCell::new(Lists::default()),
        assigned_globals: RefCell::new(HashSet::new()),
        scope: RefCell::new(None),
        lines: RefCell::new(HashMap::new()),
        lowered: RefCell::new(HashMap::new()),
        prelude_funcs: RefCell::new(HashMap::new()),
        dispatches: RefCell::new(HashMap::new()),
        prelude_params: RefCell::new(HashMap::new()),
        types: RefCell::new(Vec::new()),
        raises: Cell::new(false),
        uniform_repr: Cell::new(false),
    };
    let suite = main.module.suite().as_slice();
    lowering.declare_vars(&Bindings::of(suite))?;
    let body = lowering.body(suite)?;
    let tests = match &lowering.tests {
        Some(_) => Some(lowering.tests_driver()?),
        None => None,
    };
    let mut funcs: Vec<Func> = Vec::new();
    lowering.lower_queue(&mut funcs)?;
    // The `repr` of a uniform value may meet a `str` or an object of any
    // class, whose functions go to the end of the queue. The function of a
    // call through a value of a class waits for every call of a class,
    // and its code may convert another object.
    let mut str_repr = None;
    loop {
        if lowering.uniform_repr.get() && str_repr.is_none() {
            str_repr = Some(lowering.uniform_reprs()?);
        }
        let resolved = lowering.resolve_slots()?;
        lowering.lower_queue(&mut funcs)?;
        if !resolved && (str_repr.is_some() || !lowering.uniform_repr.get()) {
            break;
        }
    }
    debug_assert!(
        lowering.lowered.borrow().is_empty(),
        "a function without code"
    );
    let str_repr = str_repr.flatten();
    // The lowering of a later function may order an earlier one.
    for (func, ordered) in funcs.iter_mut().zip(&lowering.funcs.borrow().ordered) {
        func.ordered = *ordered;
    }
    Ok(Program {
        body,
        tests,
        vars: lowering.vars.into_inner().list,
        funcs,
        imports: lowering.imports.into_inner().list,
        classes: lowering.classes.into_inner().into_ir(),
        lists: {
            let lists = lowering.lists.into_inner();
            lists
                .args
                .into_iter()
                .zip(lists.reprs)
                .map(|(arg, repr)| ListType {
                    item: arg.rep,
                    repr,
                })
                .collect()
        },
        sites: lowering.sites.into_inner(),
        str_repr,
    })
}

/// A file of the program, parsed, with its types.
struct Loaded<'db> {
    file: File,
    module: ParsedModuleRef,
    model: SemanticModel<'db>,
    source: SourceText,
}

impl<'db> Loaded<'db> {
    fn new(db: &'db dyn Db, file: File) -> Self {
        let program_file = db.program_file(file);
        Loaded {
            file,
            module: parsed_module(db, program_file.python_file(db)).load(db),
            model: SemanticModel::new(db, program_file),
            source: source_text(db, file),
        }
    }
}

/// What the lowering reads of one file of the program.
struct Unit<'a, 'db> {
    file: File,
    model: &'a SemanticModel<'db>,
    source: &'a str,
    /// The lines of `source`, for the frames of the sites.
    lines: FileLines<'a>,
    /// The path of the file in a traceback.
    path: &'a str,
    /// The name of the module of the prelude that the file is, and `None`
    /// for the file of the program.
    prelude: Option<&'a str>,
    /// The functions of a module of the prelude, by name.
    defs: HashMap<&'a str, &'a ast::StmtFunctionDef>,
    /// The statements of the module.
    suite: &'a [ast::Stmt],
}

impl<'a, 'db> Unit<'a, 'db> {
    fn new(loaded: &'a Loaded<'db>, path: &'a str, prelude: Option<&'a str>) -> Self {
        let defs = match prelude {
            Some(_) => loaded
                .module
                .suite()
                .iter()
                .filter_map(|stmt| match stmt {
                    ast::Stmt::FunctionDef(def) => Some((def.name.as_str(), def)),
                    _ => None,
                })
                .collect(),
            None => HashMap::new(),
        };
        Unit {
            file: loaded.file,
            model: &loaded.model,
            source: loaded.source.as_str(),
            lines: FileLines::new(loaded.source.as_str()),
            path,
            prelude,
            defs,
            suite: loaded.module.suite().as_slice(),
        }
    }
}

/// The operator of a binary function of [`Lowering::dispatches`], with the
/// reps of its operands and of its result.
type DispatchKey = (String, [Rep; 2], Rep);

struct Lowering<'a, 'db> {
    /// The file of the code that the lowering is in.
    unit: Cell<&'a Unit<'a, 'db>>,
    /// The doctests of a program that tests its file.
    tests: Option<Tests<'a>>,
    /// The modules of the prelude.
    prelude: &'a [Unit<'a, 'db>],
    /// The stub of the functions of the prelude that are instructions of
    /// the module.
    intrinsics: Option<File>,
    sites: RefCell<Vec<SiteFrame>>,
    /// The variables of the module.
    vars: RefCell<Vars>,
    funcs: RefCell<Funcs<'a, 'db>>,
    imports: RefCell<Imports>,
    classes: RefCell<Classes<'a>>,
    lists: RefCell<Lists>,
    /// The globals that a function declares with `global`, which a call may
    /// assign between a test of the value and a read.
    assigned_globals: RefCell<HashSet<String>>,
    /// The function whose body the lowering is in, and `None` in the code
    /// of the module.
    scope: RefCell<Option<Scope>>,
    /// The line of the frame of each line of each function, and of the
    /// module, by the path and the name of the frame, and the number of the
    /// line.
    lines: RefCell<HashMap<(String, usize), u32>>,
    /// The functions of the prelude, which the lowering lowers at their
    /// first call, by the index of their id.
    lowered: RefCell<HashMap<usize, Func>>,
    /// Each function of the prelude that a call reached, by its file, its
    /// name and the rep of each of its type variables, so a later call
    /// skips its signature. A generic function has a copy for each rep of
    /// its type variables.
    prelude_funcs: RefCell<HashMap<PreludeKey<'a>, FuncId>>,
    /// The declared type of each parameter of each function of the prelude
    /// that a call reached, with the names of its type variables, by its
    /// file and its name.
    prelude_params: RefCell<HashMap<(File, &'a str), Rc<PreludeParams>>>,
    /// The function that tests the classes of two objects for a binary
    /// operator, by the operator, the reps of the operands and the rep of
    /// the result.
    dispatches: RefCell<HashMap<DispatchKey, FuncId>>,
    /// The rep of each type variable of the copy of a generic function of
    /// the prelude whose signature or body the lowering is in.
    types: RefCell<Vec<(String, TypeArg)>>,
    /// Whether the code of the function of the prelude that the lowering
    /// is in has an instruction that may raise.
    raises: Cell<bool>,
    /// Whether the program gives the `repr` of a uniform value.
    uniform_repr: Cell<bool>,
}

#[derive(Default)]
struct Vars {
    ids: HashMap<String, VarId>,
    list: Vec<Var>,
}

impl Vars {
    /// The variable of `name`, and its rep.
    fn find(&self, name: &str) -> Option<(VarId, Rep)> {
        let id = *self.ids.get(name)?;
        Some((id, self.get(id).rep))
    }

    fn get(&self, id: VarId) -> &Var {
        self.list.get(id.index()).expect("a variable of the scope")
    }

    fn get_mut(&mut self, id: VarId) -> &mut Var {
        self.list
            .get_mut(id.index())
            .expect("a variable of the scope")
    }

    fn add(&mut self, name: &str, rep: Rep) {
        let id = self.add_unnamed(rep);
        self.ids.insert(name.to_owned(), id);
    }

    /// A new variable that no name of the program reads.
    fn add_unnamed(&mut self, rep: Rep) -> VarId {
        self.list.push(Var {
            rep,
            flagged: false,
            ordered: false,
        });
        VarId::new(self.list.len() - 1)
    }

    /// The name of each variable, by its id.
    fn names(&self) -> Vec<(VarId, &str)> {
        let mut names: Vec<_> = self
            .ids
            .iter()
            .map(|(name, id)| (*id, name.as_str()))
            .collect();
        names.sort_by_key(|(id, _)| id.index());
        names
    }
}

/// A function of the prelude, by its file, its name and the rep of each of
/// its type variables.
type PreludeKey<'a> = (File, &'a str, Vec<TypeArg>);

/// The declared type of each parameter of a function of the prelude, and
/// the names of its type variables.
type PreludeParams = (Vec<Shape>, Vec<String>);

/// What a type variable of a copy of a generic function of the prelude
/// stands for. `classes` has the classes that a uniform value of the
/// variable may be whose `==` is other than the identity, the dataclasses
/// and the classes with `__eq__` or `__ne__`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TypeArg {
    pub(super) rep: Rep,
    pub(super) classes: Vec<ClassId>,
}

/// The types of the lists of the program.
#[derive(Default)]
struct Lists {
    /// The type of a list of each type of item.
    ids: HashMap<TypeArg, ListId>,
    /// The type of the items of each type of list, by its id.
    args: Vec<TypeArg>,
    /// The function that gives the `repr` of a list of each type, once the
    /// program writes one.
    reprs: Vec<Option<FuncId>>,
}

#[derive(Default)]
struct Funcs<'a, 'db> {
    ids: HashMap<DefId, FuncId>,
    /// The function that each name of the module binds, the last one for a
    /// name with more than one `def`.
    names: HashMap<String, FuncId>,
    /// The functions of each name with more than one `def`, in the order
    /// of the code, with the global that holds the place of the function
    /// that the name has now.
    redefined: HashMap<String, (VarId, Vec<FuncId>)>,
    sigs: Vec<Sig<'a, 'db>>,
    /// Whether the program asks which globals got a value first, by the id
    /// of the function.
    ordered: Vec<bool>,
    /// Whether a call of the function may raise, by the id of the function.
    raises: Vec<bool>,
}

impl<'a, 'db> Funcs<'a, 'db> {
    fn get(&self, id: FuncId) -> &Sig<'a, 'db> {
        self.sigs.get(id.index()).expect("a function of the module")
    }

    /// Adds the function of `def` to the end of the queue of the lowering.
    fn add(&mut self, def: DefId, sig: Sig<'a, 'db>) -> FuncId {
        let id = self.add_generated(sig, true);
        self.ids.insert(def, id);
        id
    }

    /// Adds a function that no `def` of the program names, which may raise
    /// when `raises` is `true`.
    fn add_generated(&mut self, sig: Sig<'a, 'db>, raises: bool) -> FuncId {
        let id = FuncId::new(self.sigs.len());
        self.sigs.push(sig);
        self.ordered.push(false);
        self.raises.push(raises);
        id
    }
}

/// The names of the module that an import binds.
#[derive(Default)]
struct Imports {
    ids: HashMap<String, ImportId>,
    list: Vec<Import>,
    /// What each name stands for.
    imported: HashMap<String, Imported>,
}

/// A module, or a name in a module, that an import binds.
#[derive(Clone, PartialEq, Eq)]
struct Imported {
    module: String,
    /// The name in the module, and `None` for the module itself.
    member: Option<String>,
}

impl Imports {
    fn get_mut(&mut self, id: ImportId) -> &mut Import {
        self.list
            .get_mut(id.index())
            .expect("an import of the module")
    }
}

/// What a call needs to know of a function before its body is lowered.
#[derive(Clone)]
struct Sig<'a, 'db> {
    /// The file of the function.
    unit: &'a Unit<'a, 'db>,
    /// The `def` of the function, and `None` for a function that the
    /// compiler writes, which the lowering has as a [`Func`] from the start.
    def: Option<&'a ast::StmtFunctionDef>,
    params: Vec<Rep>,
    /// The variable of the module that holds the default of each parameter
    /// that has one. The `def` gives the variable its value.
    defaults: Vec<Option<VarId>>,
    /// The docstring whose examples are the body of the function.
    doctest: Option<&'a Docstring>,
    ret: Rep,
    /// The rep of each type variable of a copy of a generic function of the
    /// prelude, in the order of the `def`.
    types: Vec<(String, TypeArg)>,
}

struct Scope {
    /// The number of parameters of the function.
    params: usize,
    /// The docstring whose examples are the body of the function.
    doctest: Option<DoctestScope>,
    /// The name of the function, which its frames in a traceback show.
    name: String,
    ret: Rep,
    /// Every name the function assigns and does not declare `global`,
    /// which makes the name local to the function, whether the variable has
    /// a rep or not.
    locals: HashSet<String>,
    /// The locals in the order of the frame of CPython.
    order: Vec<String>,
    vars: Vars,
    /// The names that the namespace of the class has, in the body of a
    /// class, where the scope has no variables.
    namespace: Vec<String>,
    /// Where the `class` statement starts, in the body of a class.
    class: Option<TextSize>,
}

impl Scope {
    /// Returns `true` if `name` is a local of the function, `false` when
    /// the function reads it from the module.
    fn is_local(&self, name: &str) -> bool {
        self.locals.contains(name)
    }
}

impl<'a, 'db> Lowering<'a, 'db> {
    fn model(&self) -> &'a SemanticModel<'db> {
        self.unit.get().model
    }

    fn source(&self) -> &'a str {
        self.unit.get().source
    }

    /// Gives each variable that the code assigns the rep of its values,
    /// which the checker infers, and declares each function of the module.
    /// A read or a call may come before the assignment, as in a loop, so
    /// every variable has a rep before the lowering of the code starts.
    fn declare_vars(&self, bindings: &Bindings<'a>) -> Result<(), Error> {
        // The lowering of a function rejects a `def` and an import in its
        // body.
        if self.scope.borrow().is_none() {
            for (name, alias, from) in &bindings.imports {
                self.declare_import(name, alias, *from)?;
            }
            for class in &bindings.classes {
                self.declare_class(class)?;
            }
            self.declare_fields()?;
            self.declare_methods()?;
            for def in &bindings.defs {
                self.declare_func(def)?;
                self.declare_globals(def)?;
            }
        }
        let keep = |name: &str| {
            self.scope
                .borrow()
                .as_ref()
                .is_none_or(|scope| scope.is_local(name))
        };
        self.declare_assigned(&bindings.assignments, keep)?;
        self.declare_loops(&bindings.loops, keep)
    }

    /// Declares in the module each global that `def` assigns, since the
    /// code of the module may read it.
    fn declare_globals(&self, def: &'a ast::StmtFunctionDef) -> Result<(), Error> {
        let bindings = Bindings::of(&def.body);
        self.assigned_globals
            .borrow_mut()
            .extend(bindings.globals.iter().map(|name| (*name).to_owned()));
        let keep = |name: &str| bindings.globals.contains(name);
        self.declare_assigned(&bindings.assignments, keep)?;
        self.declare_loops(&bindings.loops, keep)
    }

    /// Declares the target of each `for` of `loops` whose name `keep`
    /// accepts. The annotation of the name gives its rep, and the items of
    /// the iterable give it otherwise, which are `int`s for a `range`.
    fn declare_loops(
        &self,
        loops: &[(&'a ast::Expr, &'a ast::Expr)],
        keep: impl Fn(&str) -> bool,
    ) -> Result<(), Error> {
        for (target, iter) in loops {
            let rep = match slot_shape(self.model(), target) {
                Some(shape) => self.rep_of(Some(shape)),
                None if self.range_call(iter).is_some() => Some(Rep::Int),
                None => match self.rep_of(expr_shape(self.model(), iter)) {
                    Some(Rep::List(list)) => Some(self.list_item(list)),
                    Some(Rep::Str) => Some(Rep::Str),
                    _ => None,
                },
            };
            if let ast::Expr::Name(name) = target
                && keep(name.id.as_str())
                && let Some(rep) = rep
            {
                self.declare_var(name.id.as_str(), rep, iter)?;
            }
        }
        Ok(())
    }

    /// Declares each variable of `assigned` whose name `keep` accepts, in
    /// the scope that the lowering is in.
    fn declare_assigned(
        &self,
        assigned: &[Assigned<'a>],
        keep: impl Fn(&str) -> bool,
    ) -> Result<(), Error> {
        for (targets, value, annotated) in assigned {
            for target in *targets {
                // An annotation declares the rep of the variable, and the
                // value gives it otherwise. The lowering of the assignment
                // rejects a value without a rep.
                let declared = match annotated {
                    Some(annotated) => declared_shape(self.model(), annotated),
                    None => slot_shape(self.model(), target),
                };
                let shape = declared.or_else(|| expr_shape(self.model(), value));
                // The checker gives a copy of a name of the module the type
                // of the name at the copy. That type may be narrower than
                // the variable, or empty for a name that the module assigns
                // only in a branch that the checker deems dead.
                let copied = self
                    .module_copy(targets, value)
                    .and_then(|(_, from)| self.vars.borrow().find(from))
                    .map(|(_, rep)| rep);
                let rep = match (self.rep_of(shape), copied) {
                    (Some(rep), Some(copied)) => Some(
                        self.join(rep, copied)
                            .ok_or_else(|| unsupported(value, "a value of two types"))?,
                    ),
                    (rep, copied) => rep.or(copied),
                };
                if let ast::Expr::Name(name) = target
                    && keep(name.id.as_str())
                    && let Some(rep) = rep
                {
                    self.declare_var(name.id.as_str(), rep, value)?;
                }
            }
        }
        Ok(())
    }

    fn declare_var(&self, name: &str, rep: Rep, value: &ast::Expr) -> Result<(), Error> {
        check_self(name, rep, value)?;
        let mut scope = self.scope.borrow_mut();
        let mut module = self.vars.borrow_mut();
        let vars = match scope.as_mut() {
            Some(scope) => &mut scope.vars,
            None => {
                if self.funcs.borrow().names.contains_key(name) {
                    return Err(unsupported(value, "a name of a function and of a variable"));
                }
                if self.imports.borrow().ids.contains_key(name) {
                    return Err(unsupported(value, "a name of an import and of a variable"));
                }
                if self.classes.borrow().names.contains_key(name) {
                    return Err(unsupported(value, "a name of a class and of a variable"));
                }
                &mut *module
            }
        };
        match vars.find(name) {
            Some((_, declared)) if !self.holds(declared, rep) => {
                Err(unsupported(value, "a variable with values of two types"))
            }
            Some(_) => Ok(()),
            None => {
                vars.add(name, rep);
                Ok(())
            }
        }
    }

    fn declare_func(&self, def: &'a ast::StmtFunctionDef) -> Result<(), Error> {
        let (id, params, ret) = self.signature(def, None)?;
        let name = def.name.as_str();
        if self.vars.borrow().ids.contains_key(name) {
            return Err(unsupported(def, "a name of a function and of a variable"));
        }
        if self.imports.borrow().ids.contains_key(name) {
            return Err(unsupported(def, "a name of an import and of a function"));
        }
        if self.classes.borrow().names.contains_key(name) {
            return Err(unsupported(def, "a name of a class and of a function"));
        }
        let mut funcs = self.funcs.borrow_mut();
        let mut vars = self.vars.borrow_mut();
        let defaults = params_of(def)
            .zip(&params)
            .map(|(param, rep)| param.default.as_ref().map(|_| vars.add_unnamed(*rep)))
            .collect();
        let doctest = self.tests.as_ref().and_then(|tests| {
            tests
                .docstrings
                .iter()
                .find(|docstring| docstring.function == name)
        });
        let func = funcs.add(
            id,
            Sig {
                unit: self.unit.get(),
                def: Some(def),
                params,
                defaults,
                doctest,
                ret,
                types: Vec::new(),
            },
        );
        if let Some(first) = funcs.names.insert(name.to_owned(), func) {
            let (_, defs) = funcs.redefined.entry(name.to_owned()).or_insert_with(|| {
                let current = vars.add_unnamed(Rep::Int);
                vars.get_mut(current).flagged = true;
                (current, vec![first])
            });
            defs.push(func);
        }
        Ok(())
    }

    /// The function of `def`, a `def` of the module.
    fn def_func(&self, def: &ast::StmtFunctionDef) -> FuncId {
        let funcs = self.funcs.borrow();
        let name = def.name.as_str();
        let last = *funcs.names.get(name).expect("a declared function");
        funcs
            .redefined
            .get(name)
            .and_then(|(_, defs)| {
                defs.iter()
                    .copied()
                    .find(|func| funcs.get(*func).def.is_some_and(|d| std::ptr::eq(d, def)))
            })
            .unwrap_or(last)
    }

    /// The id of `def`, a function of the file that the lowering is in,
    /// with the rep of each parameter and of the result. The first
    /// parameter of a method of `receiver` holds an object of the class.
    fn signature(
        &self,
        def: &ast::StmtFunctionDef,
        receiver: Option<ClassId>,
    ) -> Result<(DefId, Vec<Rep>, Rep), Error> {
        // Only the prelude has generic functions, whose copy has a rep for
        // each type variable.
        let generic = def.type_params.is_some() && !self.in_prelude();
        if def.is_async || !def.decorator_list.is_empty() || generic {
            return Err(unsupported(def, "this def"));
        }
        let parameters = &def.parameters;
        if !parameters.posonlyargs.is_empty()
            || parameters.vararg.is_some()
            || parameters.kwarg.is_some()
        {
            return Err(unsupported(&**parameters, "these parameters"));
        }
        if let Some(param) = params_of(def).nth(MAX_PARAMS) {
            return Err(unsupported(param, "a def with more than 1000 parameters"));
        }
        if receiver.is_some()
            && parameters
                .args
                .first()
                .is_none_or(|param| param.default.is_some())
        {
            return Err(unsupported(&**parameters, "a method without self"));
        }
        let info = function_info(self.model(), def).ok_or_else(|| unsupported(def, "this def"))?;
        let params = params_of(def)
            .zip(&info.params)
            .enumerate()
            .map(|(i, (param, info))| {
                if let Some(class) = receiver.filter(|_| i == 0) {
                    return Ok(Rep::Object(class));
                }
                let rep = self
                    .rep_of(Some(info.shape.clone()))
                    .ok_or_else(|| unsupported(param, "a parameter of this type"))?;
                check_self(param.name(), rep, param)?;
                Ok(rep)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ret = match info.ret {
            Shape::None => Rep::None,
            shape => self.rep_of(Some(shape)).ok_or_else(|| match &def.returns {
                Some(returns) => unsupported(&**returns, "a result of this type"),
                None => unsupported(&def.name, "a result of this type"),
            })?,
        };
        Ok((info.id, params, ret))
    }

    /// Declares `name`, which `alias` imports in the module, from the
    /// module `from` in a `from` import. The suggestion of a `NameError`
    /// would need the attributes of a `self` that an import binds.
    fn declare_import(
        &self,
        name: &str,
        alias: &ast::Alias,
        from: Option<&str>,
    ) -> Result<(), Error> {
        if name == "self" {
            return Err(unsupported(alias, "an import that binds self"));
        }
        let imported = match from {
            Some(module) => Imported {
                module: module.to_owned(),
                member: Some(alias.name.to_string()),
            },
            None => Imported {
                module: alias.name.to_string(),
                member: None,
            },
        };
        let mut imports = self.imports.borrow_mut();
        match imports.imported.get(name) {
            Some(declared) if *declared != imported => {
                return Err(unsupported(alias, "a name of two imports"));
            }
            Some(_) => {}
            None => {
                imports.imported.insert(name.to_owned(), imported);
            }
        }
        if !imports.ids.contains_key(name) {
            let id = ImportId::new(imports.list.len());
            imports.ids.insert(name.to_owned(), id);
            imports.list.push(Import { ordered: false });
        }
        Ok(())
    }

    /// The body of the function, lowered in its own scope and in its file,
    /// with the reps of its type variables.
    fn func(&self, id: FuncId) -> Result<Func, Error> {
        let sig = self.funcs.borrow().get(id).clone();
        let outer = self.unit.replace(sig.unit);
        let types = self.types.replace(sig.types.clone());
        let func = self.func_of(&sig);
        self.types.replace(types);
        self.unit.set(outer);
        func
    }

    fn func_of(&self, sig: &Sig<'a, 'db>) -> Result<Func, Error> {
        let def = sig.def.expect("a function with a def");
        let mut vars = Vars::default();
        let mut locals = HashSet::new();
        let mut order = Vec::new();
        for (param, rep) in params_of(def).zip(&sig.params) {
            let name = param.parameter.name.as_str();
            vars.add(name, *rep);
            locals.insert(name.to_owned());
            order.push(name);
        }
        let bindings = Bindings::of(&def.body);
        locals.extend(bindings.assigned.iter().map(|name| (*name).to_owned()));
        locals.retain(|name| !bindings.globals.contains(name.as_str()));
        let mut seen = HashSet::new();
        let order = order
            .into_iter()
            .chain(bindings.order.iter().copied())
            .filter(|name| locals.contains(*name) && seen.insert(*name))
            .map(str::to_owned)
            .collect();
        *self.scope.borrow_mut() = Some(Scope {
            params: sig.params.len(),
            doctest: sig.doctest.map(|docstring| DoctestScope {
                owner: docstring.owner.clone(),
                example: None,
            }),
            name: def.name.to_string(),
            ret: sig.ret,
            locals,
            order,
            vars,
            namespace: Vec::new(),
            class: None,
        });
        self.declare_vars(&bindings)?;
        let body = match sig.doctest {
            Some(docstring) => self.doctest_body(def, docstring)?,
            None => self.body(&def.body)?,
        };
        if sig.ret != Rep::None && !always_returns(&def.body) {
            return Err(unsupported(
                &def.name,
                "a function that may end without a return",
            ));
        }
        let scope = self.scope.take().expect("the scope of the function");
        Ok(Func {
            alias: None,
            ordered: false,
            frame: !self.in_prelude(),
            params: u32::try_from(sig.params.len()).expect("fewer than 2^32 parameters"),
            vars: scope.vars.list,
            ret: sig.ret,
            body,
        })
    }

    /// A new variable of `rep` that no name of the program reads, local to
    /// the function that the lowering is in, or global in the module.
    fn hidden_var(&self, rep: Rep) -> Place {
        match self.scope.borrow_mut().as_mut() {
            Some(scope) => Place::Local(scope.vars.add_unnamed(rep)),
            None => Place::Global(self.vars.borrow_mut().add_unnamed(rep)),
        }
    }

    /// A function that the compiler writes, without a frame in a
    /// traceback, with a parameter of each rep of `params` and the code of
    /// `body`, which reads the parameters as the first locals.
    fn generated(&self, params: Vec<Rep>, ret: Rep, body: Vec<Stmt>) -> FuncId {
        let id = self.reserve(params, ret);
        self.define(id, false, body);
        id
    }

    /// A function that the compiler writes, with a parameter of each rep of
    /// `params`, whose code [`Self::define`] gives later. The code may call
    /// the function.
    fn reserve(&self, params: Vec<Rep>, ret: Rep) -> FuncId {
        let defaults = params.iter().map(|_| None).collect();
        self.reserve_with(params, ret, defaults, true)
    }

    /// [`Self::reserve`] of a function with the global of each default in
    /// `defaults`, which may raise when `raises` is `true`.
    fn reserve_with(
        &self,
        params: Vec<Rep>,
        ret: Rep,
        defaults: Vec<Option<VarId>>,
        raises: bool,
    ) -> FuncId {
        self.funcs.borrow_mut().add_generated(
            Sig {
                unit: self.unit.get(),
                def: None,
                defaults,
                doctest: None,
                params,
                ret,
                types: Vec::new(),
            },
            raises,
        )
    }

    /// Gives `id`, a function of [`Self::reserve`], the code of `body`,
    /// which reads the parameters as the first locals. A function with a
    /// `frame` has a frame in a traceback, from the sites of its code.
    fn define(&self, id: FuncId, frame: bool, body: Vec<Stmt>) {
        self.define_with(id, frame, &[], body);
    }

    /// [`Self::define`] of a function with a local of each rep of
    /// `locals` after the parameters.
    fn define_with(&self, id: FuncId, frame: bool, locals: &[Rep], body: Vec<Stmt>) {
        let sig = self.funcs.borrow().get(id).clone();
        let vars = sig
            .params
            .iter()
            .chain(locals)
            .map(|rep| Var {
                rep: *rep,
                flagged: false,
                ordered: false,
            })
            .collect();
        let func = Func {
            alias: None,
            ordered: false,
            frame,
            params: u32::try_from(sig.params.len()).expect("fewer than 2^32 parameters"),
            vars,
            ret: sig.ret,
            body,
        };
        self.lowered.borrow_mut().insert(id.index(), func);
    }

    /// Makes `id`, a function of [`Self::reserve`], call `target` in its
    /// place.
    fn alias(&self, id: FuncId, target: FuncId) {
        let sig = self.funcs.borrow().get(id).clone();
        let func = Func {
            alias: Some(target),
            ordered: false,
            frame: false,
            params: u32::try_from(sig.params.len()).expect("fewer than 2^32 parameters"),
            vars: sig
                .params
                .iter()
                .map(|rep| Var {
                    rep: *rep,
                    flagged: false,
                    ordered: false,
                })
                .collect(),
            ret: sig.ret,
            body: Vec::new(),
        };
        self.lowered.borrow_mut().insert(id.index(), func);
    }

    /// The rep of the values of `shape`, and `None` when the compiler has no
    /// rep for them.
    fn rep_of(&self, shape: Option<Shape>) -> Option<Rep> {
        match shape? {
            Shape::Int | Shape::IntLit(_) => Some(Rep::Int),
            Shape::Float => Some(Rep::Float),
            Shape::Bool | Shape::BoolLit(_) => Some(Rep::Bool),
            Shape::Str | Shape::StrLit(_) => Some(Rep::Str),
            // A builder of the prelude is a `str` that its code still
            // writes.
            Shape::Instance(class, _)
                if Some(class.file) == self.intrinsics && class.name == "StrBuilder" =>
            {
                Some(Rep::Str)
            }
            Shape::Instance(class, _) if self.is_object(&class) => Some(Rep::Uniform),
            Shape::Instance(class, _) | Shape::EnumMember(class, _) => self
                .classes
                .borrow()
                .ids
                .get(&class)
                .copied()
                .map(Rep::Object),
            Shape::List(item) => {
                let rep = self.rep_of(Some((*item).clone()))?;
                let classes = match rep {
                    Rep::Uniform => self.uniform_eq_classes(Some(*item)),
                    _ => Vec::new(),
                };
                Some(Rep::List(self.list_of(TypeArg { rep, classes })))
            }
            Shape::TypeVar(name) => self
                .types
                .borrow()
                .iter()
                .find(|(var, _)| *var == name)
                .map(|(_, arg)| arg.rep),
            Shape::Union(shapes) => {
                let reps = shapes
                    .into_iter()
                    .map(|shape| match shape {
                        Shape::None => Some(Rep::None),
                        shape => self.rep_of(Some(shape)),
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.join_all(reps)
            }
            _ => None,
        }
    }

    /// The type of a list whose items have the type `arg`, which the first
    /// call for it creates.
    fn list_of(&self, arg: TypeArg) -> ListId {
        let mut lists = self.lists.borrow_mut();
        if let Some(list) = lists.ids.get(&arg) {
            return *list;
        }
        let list = ListId::new(lists.args.len());
        lists.args.push(arg.clone());
        lists.reprs.push(None);
        lists.ids.insert(arg, list);
        list
    }

    /// The type of each item of a list of the type `list`.
    fn list_arg(&self, list: ListId) -> TypeArg {
        self.lists
            .borrow()
            .args
            .get(list.index())
            .expect("a type of list of the program")
            .clone()
    }

    /// The rep of each item of a list of the type `list`.
    fn list_item(&self, list: ListId) -> Rep {
        self.lists
            .borrow()
            .args
            .get(list.index())
            .expect("a type of list of the program")
            .rep
    }

    /// Lowers the functions of the queue after the ones in `funcs`. The
    /// lowering of a function may declare another one, which goes to the
    /// end of the queue.
    /// A function that the compiler writes and has no code yet, as the
    /// function of a slot, keeps its place with an empty function until
    /// the code comes.
    fn lower_queue(&self, funcs: &mut Vec<Func>) -> Result<(), Error> {
        for (i, func) in funcs.iter_mut().enumerate() {
            if let Some(lowered) = self.lowered.borrow_mut().remove(&i) {
                *func = lowered;
            }
        }
        while funcs.len() < self.funcs.borrow().sigs.len() {
            let id = FuncId::new(funcs.len());
            let lowered = self.lowered.borrow_mut().remove(&id.index());
            let has_def = self.funcs.borrow().get(id).def.is_some();
            funcs.push(match lowered {
                Some(func) => func,
                None if has_def => self.func(id)?,
                None => Func {
                    alias: None,
                    ordered: false,
                    frame: false,
                    params: 0,
                    vars: Vec::new(),
                    ret: Rep::None,
                    body: Vec::new(),
                },
            });
        }
        Ok(())
    }

    /// Makes the function that gives the `repr` of a `str` and of an object
    /// of each class, which the `repr` of a uniform value calls, and
    /// returns the one of the `str`.
    fn uniform_reprs(&self) -> Result<Option<FuncId>, Error> {
        let count = self.classes.borrow().list.len();
        for i in 0..count {
            self.class_conversion(ClassId::new(i), true);
        }
        self.prelude_named("str", "repr_of")
    }

    /// A new site for the instruction at `range`, in the code that the
    /// lowering is in.
    fn site(&self, range: TextRange) -> Site {
        if self.in_prelude() {
            self.raises.set(true);
            return Site::NONE;
        }
        let unit = self.unit.get();
        let number = traceback::line(&unit.lines, range);
        let (path, name, number) = match self.scope.borrow().as_ref() {
            None => (unit.path.to_owned(), "<module>".to_owned(), number),
            // The code of an example is a file of its own, as in the
            // `doctest` of CPython.
            Some(Scope {
                doctest:
                    Some(DoctestScope {
                        owner,
                        example: Some((index, first)),
                    }),
                ..
            }) => (
                format!("<doctest {owner}[{index}]>"),
                "<module>".to_owned(),
                number + 1 - first,
            ),
            Some(scope) => (unit.path.to_owned(), scope.name.clone(), number),
        };
        let text = traceback::frame(&path, &name, number, &unit.lines, range);
        self.add_site(format!("{path}\n{name}"), number, text)
    }

    /// A new site for the instruction at `range` in the code of the module,
    /// whose frame comes with the frames `after`, as the frames of the
    /// standard library that a traceback shows after the frame of the
    /// module.
    fn site_with(&self, range: TextRange, after: &str) -> Site {
        let unit = self.unit.get();
        let number = traceback::line(&unit.lines, range);
        let text = traceback::frame(unit.path, "<module>", number, &unit.lines, range);
        self.add_site(format!("{}\n<module>", unit.path), number, text + after)
    }

    /// A new site with the frame `text`, at the line `number` of the file
    /// and the function that `name` gives.
    fn add_site(&self, name: String, number: usize, text: String) -> Site {
        let mut lines = self.lines.borrow_mut();
        let count = u32::try_from(lines.len()).expect("fewer than 2^32 lines");
        let line = *lines.entry((name, number)).or_insert(count);
        let mut sites = self.sites.borrow_mut();
        sites.push(SiteFrame { text, line });
        Site::new(sites.len() - 1)
    }
}

/// The parameters of `def` in the order of the arguments of a call of the
/// function, which is the order of the checker and of a frame of CPython.
/// A keyword-only parameter comes after the others.
fn params_of(def: &ast::StmtFunctionDef) -> impl Iterator<Item = &ast::ParameterWithDefault> {
    let parameters = &def.parameters;
    parameters.args.iter().chain(&parameters.kwonlyargs)
}

/// The most parameters that a wasm engine accepts in a function.
const MAX_PARAMS: usize = 1000;

impl Lowering<'_, '_> {
    /// `value` as a value of `rep`, and `None` when a variable of `rep`
    /// does not hold it.
    fn coerce(&self, value: Expr, rep: Rep) -> Option<Expr> {
        let from = value.rep();
        if from == rep {
            return Some(value);
        }
        self.holds(rep, from).then(|| Expr::Widen {
            value: Box::new(value),
            rep,
        })
    }

    /// Returns `true` if a variable of `rep` holds the values of `value`,
    /// `false` otherwise. A variable of a class holds an object of a class
    /// that descends from it.
    fn holds(&self, rep: Rep, value: Rep) -> bool {
        match (rep, value) {
            (Rep::Object(class), Rep::Object(other))
            | (Rep::ObjectOrNone(class), Rep::Object(other) | Rep::ObjectOrNone(other)) => {
                self.descends(other, class)
            }
            (Rep::ObjectOrNone(_), Rep::None) => true,
            (Rep::StrOrNone, _) => matches!(value, Rep::Str | Rep::None | Rep::StrOrNone),
            (Rep::ListOrNone(list), _) => {
                matches!(value, Rep::None) || value == Rep::List(list) || value == rep
            }
            // The dispatch of a uniform value knows no list.
            (Rep::Uniform, _) => value.list().is_none(),
            _ => rep == value,
        }
    }

    /// The narrowest rep that holds the values of `a` and the values of
    /// `b`, and `None` when a list meets a value of another type, which
    /// no rep holds yet.
    fn join(&self, a: Rep, b: Rep) -> Option<Rep> {
        let pair = [a, b];
        Some(if self.holds(a, b) {
            a
        } else if self.holds(b, a) {
            b
        } else if pair
            .iter()
            .all(|rep| matches!(rep, Rep::Str | Rep::None | Rep::StrOrNone))
        {
            Rep::StrOrNone
        } else if let Some(class) = pair.iter().filter_map(|rep| rep.class()).find(|class| {
            pair.iter().all(|rep| {
                *rep == Rep::None
                    || rep
                        .class()
                        .is_some_and(|other| self.descends(other, *class))
            })
        }) {
            Rep::ObjectOrNone(class)
        } else if let Some(list) = a.list().or(b.list())
            && pair
                .iter()
                .all(|rep| *rep == Rep::None || rep.list() == Some(list))
        {
            Rep::ListOrNone(list)
        } else if pair.iter().all(|rep| rep.list().is_none()) {
            Rep::Uniform
        } else {
            return None;
        })
    }

    /// The join of all of `reps`, and `None` when `reps` is empty or has
    /// no join.
    fn join_all(&self, reps: impl IntoIterator<Item = Rep>) -> Option<Rep> {
        let mut reps = reps.into_iter();
        let first = reps.next()?;
        reps.try_fold(first, |a, b| self.join(a, b))
    }
}

/// Refuses a variable `self` of a union or of a class. The suggestion of a
/// `NameError` looks for an attribute in the value of `self`, and the
/// compiler knows the attributes of each builtin rep but not which one a
/// union holds, nor the attributes of a class.
fn check_self(name: &str, rep: Rep, node: &impl Ranged) -> Result<(), Error> {
    match rep {
        _ if name != "self" => Ok(()),
        Rep::StrOrNone | Rep::Uniform => Err(unsupported(node, "a variable self of a union")),
        Rep::Object(_) | Rep::ObjectOrNone(_) => {
            Err(unsupported(node, "a variable self of a class"))
        }
        Rep::List(_) | Rep::ListOrNone(_) => Err(unsupported(node, "a variable self of a list")),
        Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::None => Ok(()),
    }
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported {
        range: node.range(),
        what: what.to_owned(),
    }
}
