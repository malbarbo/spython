//! From the syntax tree and the types of the checker to the IR.

mod call;
mod capture;
mod class;
mod closure;
mod comprehension;
mod consumer;
mod doctest;
mod expr;
mod flow;
mod generator;
mod hash;
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
    ArrayId, ArrayType, ClassId, Expr, Func, FuncId, FuncType, FuncTypeId, Import, ImportId, Place,
    Program, Rep, Site, SiteFrame, Stmt, Var, VarId,
};
use crate::{Docstring, Error, Unsupported};
use capture::Captures;
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
        arrays: RefCell::new(Arrays::default()),
        func_types: RefCell::new(FuncTypes::default()),
        values: RefCell::new(HashMap::new()),
        inner_defs: RefCell::new(HashMap::new()),
        assigned_globals: RefCell::new(HashSet::new()),
        scope: RefCell::new(None),
        comprehension: RefCell::default(),
        substitutes: RefCell::default(),
        lines: RefCell::new(HashMap::new()),
        lowered: RefCell::new(HashMap::new()),
        prelude_funcs: RefCell::new(HashMap::new()),
        dispatches: RefCell::new(HashMap::new()),
        prelude_params: RefCell::new(HashMap::new()),
        types: RefCell::new(Vec::new()),
        raises: Cell::new(false),
        uniform_repr: Cell::new(false),
        annotation: Cell::new(None),
        uniform_hash: Cell::new(None),
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
    // and its code may convert another object. The code of the `__eq__` of
    // a copy may make another copy.
    let mut str_repr = None;
    // The classes up to here have the function of their `repr`.
    let mut with_repr = 0;
    loop {
        let classes = lowering.classes.borrow().list.len();
        if lowering.uniform_repr.get() && (str_repr.is_none() || with_repr < classes) {
            str_repr = Some(lowering.uniform_reprs(with_repr..classes)?);
            with_repr = classes;
        }
        let resolved = lowering.resolve_slots()?;
        let eqs = lowering.declare_copy_eqs();
        lowering.lower_queue(&mut funcs)?;
        let classes = lowering.classes.borrow().list.len();
        let reprs = lowering.uniform_repr.get() && (str_repr.is_none() || with_repr < classes);
        if !resolved && !eqs && !reprs {
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
        arrays: {
            let arrays = lowering.arrays.into_inner();
            arrays
                .args
                .into_iter()
                .map(|arg| ArrayType { item: arg.rep })
                .collect()
        },
        func_types: lowering.func_types.into_inner().list,
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
    /// The classes of a module of the prelude, by name.
    classes: HashMap<&'a str, &'a ast::StmtClassDef>,
    /// The statements of the module.
    suite: &'a [ast::Stmt],
}

impl<'a, 'db> Unit<'a, 'db> {
    fn new(loaded: &'a Loaded<'db>, path: &'a str, prelude: Option<&'a str>) -> Self {
        let suite = match prelude {
            Some(_) => loaded.module.suite().as_slice(),
            None => &[],
        };
        let defs = suite
            .iter()
            .filter_map(|stmt| match stmt {
                ast::Stmt::FunctionDef(def) => Some((def.name.as_str(), def)),
                _ => None,
            })
            .collect();
        let classes = suite
            .iter()
            .filter_map(|stmt| match stmt {
                ast::Stmt::ClassDef(def) => Some((def.name.as_str(), def)),
                _ => None,
            })
            .collect();
        Unit {
            file: loaded.file,
            model: &loaded.model,
            source: loaded.source.as_str(),
            lines: FileLines::new(loaded.source.as_str()),
            path,
            prelude,
            defs,
            classes,
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
    arrays: RefCell<Arrays>,
    func_types: RefCell<FuncTypes>,
    /// The function that gives the value of each function of the module
    /// whose name the program reads as a value.
    values: RefCell<HashMap<FuncId, FuncId>>,
    /// The type of function of each `def` inside a function.
    inner_defs: RefCell<HashMap<DefId, FuncTypeId>>,
    /// The globals that a function declares with `global`, which a call may
    /// assign between a test of the value and a read.
    assigned_globals: RefCell<HashSet<String>>,
    /// The function whose body the lowering is in, and `None` in the code
    /// of the module.
    scope: RefCell<Option<Scope>>,
    /// The comprehensions around the code of the module.
    comprehension: RefCell<Comprehension>,
    /// The value of each part of the first iterable of the generator
    /// expression whose code the lowering is in, by its range, which the
    /// code around the generator runs.
    substitutes: RefCell<Vec<(TextRange, Expr)>>,
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
    /// The file and the range of the value of the annotated assignment of
    /// the prelude that the lowering is in, when the value is a call, with
    /// the rep of its annotation. The checker gives no type to a call of
    /// `array_new`, which takes the type of its array from the annotation
    /// when the call is the value.
    annotation: Cell<Option<(File, TextRange, Rep)>>,
    /// The function that gives the `hash` of a uniform value, once a table
    /// needs it.
    uniform_hash: Cell<Option<FuncId>>,
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
            cell: false,
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

/// The types of the arrays of the program.
#[derive(Default)]
struct Arrays {
    /// The type of an array of each type of item.
    ids: HashMap<TypeArg, ArrayId>,
    /// The type of the items of each type of array, by its id.
    args: Vec<TypeArg>,
}

/// The types of the functions that the program holds as values.
#[derive(Default)]
struct FuncTypes {
    ids: HashMap<FuncType, FuncTypeId>,
    list: Vec<FuncType>,
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
    /// The variables of the function that live in a cell, and what each
    /// function inside it captures.
    inner: Captures,
    /// The variables that the function captures from the function around
    /// it, in the order of [`Func::captures`].
    captures: Vec<ScopeCapture>,
    comprehension: Comprehension,
}

/// The comprehensions around the code that the lowering is in, in the
/// function or in the module.
#[derive(Default)]
struct Comprehension {
    /// Each name that a comprehension binds, with its variable and its rep,
    /// from the outermost comprehension in.
    vars: Vec<(String, Place, Rep)>,
    /// The number of comprehensions around the code. A target of a loop
    /// inside one binds a new variable.
    open: usize,
}

/// A variable that a function captures from the function around it.
#[derive(Clone)]
struct ScopeCapture {
    name: String,
    rep: Rep,
    /// Whether the variable lives in a cell.
    cell: bool,
    /// Whether the function may read the variable before it has a value.
    unbound: bool,
}

impl Scope {
    /// The scope of a function named `name` whose locals are `order`, of
    /// which the first `params` are its parameters.
    fn function(
        name: String,
        params: usize,
        ret: Rep,
        vars: Vars,
        order: Vec<String>,
        captures: Vec<ScopeCapture>,
    ) -> Scope {
        Scope {
            params,
            doctest: None,
            name,
            ret,
            locals: order.iter().cloned().collect(),
            order,
            vars,
            namespace: Vec::new(),
            class: None,
            inner: Captures::default(),
            captures,
            comprehension: Comprehension::default(),
        }
    }

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
        // The module declares its imports, classes and functions. Inside a
        // function, a `def` is a variable.
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
        } else {
            for def in &bindings.defs {
                self.declare_inner(def)?;
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
            let targets = match dict_loop(self.model(), target, iter) {
                Some(targets) => targets
                    .into_iter()
                    .map(|(target, shape)| {
                        let shape = slot_shape(self.model(), target).unwrap_or(shape);
                        (target, self.rep_of(Some(shape)))
                    })
                    .collect(),
                None => {
                    let rep = match slot_shape(self.model(), target) {
                        Some(shape) => self.rep_of(Some(shape)),
                        None if self.range_call(iter).is_some() => Some(Rep::Int),
                        None => match self.rep_of(expr_shape(self.model(), iter)) {
                            Some(Rep::Str) => Some(Rep::Str),
                            Some(rep) => self.list_item_of(rep),
                            None => None,
                        },
                    };
                    vec![(*target, rep)]
                }
            };
            for (target, rep) in targets {
                if let ast::Expr::Name(name) = target
                    && keep(name.id.as_str())
                    && let Some(rep) = rep
                {
                    self.declare_var(name.id.as_str(), rep, iter)?;
                }
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

    fn declare_var(&self, name: &str, rep: Rep, value: &impl Ranged) -> Result<(), Error> {
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

    /// Declares the variable of `def`, a `def` inside a function, which
    /// holds a value of the function.
    fn declare_inner(&self, def: &'a ast::StmtFunctionDef) -> Result<(), Error> {
        let (id, params, ret) = self.signature(def, None)?;
        let ty = self.func_type(params, ret);
        self.inner_defs.borrow_mut().insert(id, ty);
        let local = self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.is_local(def.name.as_str()));
        if !local {
            return Err(unsupported(&def.name, "a def of a global in a function"));
        }
        self.declare_var(def.name.as_str(), Rep::Func(ty), &def.name)
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
        check_params(parameters)?;
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
        let (mut scope, bindings) = self.def_scope(def, &sig.params, sig.ret, Vec::new());
        scope.doctest = sig.doctest.map(|docstring| DoctestScope {
            owner: docstring.owner.clone(),
            example: None,
        });
        *self.scope.borrow_mut() = Some(scope);
        let body = self.def_body(def, &bindings, sig.ret, sig.doctest)?;
        let scope = self.scope.take().expect("the scope of the function");
        Ok(Func::new(
            !self.in_prelude(),
            sig.params.len(),
            scope.vars.list,
            sig.ret,
            body,
        ))
    }

    /// The scope of the body of `def`, a function with parameters of
    /// `params`, a result of `ret` and the captures `captures`, together
    /// with what the body binds.
    fn def_scope(
        &self,
        def: &'a ast::StmtFunctionDef,
        params: &[Rep],
        ret: Rep,
        captures: Vec<ScopeCapture>,
    ) -> (Scope, Bindings<'a>) {
        let mut vars = Vars::default();
        let mut locals = HashSet::new();
        let mut order = Vec::new();
        for (param, rep) in params_of(def).zip(params) {
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
        let scope = Scope {
            locals,
            ..Scope::function(
                def.name.to_string(),
                params.len(),
                ret,
                vars,
                order,
                captures,
            )
        };
        (scope, bindings)
    }

    /// The code of `def`, whose result has `ret`, in the scope of its body,
    /// which `bindings` gives. The body of a function of a docstring runs
    /// the examples of `doctest`.
    fn def_body(
        &self,
        def: &'a ast::StmtFunctionDef,
        bindings: &Bindings<'a>,
        ret: Rep,
        doctest: Option<&'a Docstring>,
    ) -> Result<Vec<Stmt>, Error> {
        self.declare_vars(bindings)?;
        self.declare_cells(def)?;
        let body = match doctest {
            Some(docstring) => self.doctest_body(def, docstring)?,
            None => self.body(&def.body)?,
        };
        if ret != Rep::None && !always_returns(&def.body) {
            return Err(unsupported(
                &def.name,
                "a function that may end without a return",
            ));
        }
        Ok(body)
    }

    /// Finds what the functions inside `def` capture from it, and puts each
    /// variable that needs one in a cell.
    fn declare_cells(&self, def: &ast::StmtFunctionDef) -> Result<(), Error> {
        let params: Vec<&str> = params_of(def)
            .map(|param| param.parameter.name.as_str())
            .collect();
        let mut scope = self.scope.borrow_mut();
        let scope = scope.as_mut().expect("the scope of the function");
        let inner = capture::captures(&params, &def.body, &|name| scope.is_local(name));
        for name in &inner.cells {
            if let Some((id, rep)) = scope.vars.find(name) {
                if rep == Rep::None {
                    return Err(unsupported(&def.name, "a capture of a variable of None"));
                }
                scope.vars.get_mut(id).cell = true;
            }
        }
        scope.inner = inner;
        Ok(())
    }

    /// A new variable of `rep` that no name of the program reads, local to
    /// the function that the lowering is in, or global in the module.
    fn hidden_var(&self, rep: Rep) -> Place {
        match self.scope.borrow_mut().as_mut() {
            Some(scope) => Place::Local(scope.vars.add_unnamed(rep)),
            None => Place::Global(self.vars.borrow_mut().add_unnamed(rep)),
        }
    }

    /// A new hidden variable that holds `value`, by an assignment at the end
    /// of `stmts`, with its rep.
    fn hold(&self, stmts: &mut Vec<Stmt>, value: Expr) -> (Place, Rep) {
        let rep = value.rep();
        let var = self.hidden_var(rep);
        stmts.push(Stmt::Assign {
            targets: vec![var],
            value,
        });
        (var, rep)
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
        let func = self.func_of_sig(id, frame, locals, body);
        self.lowered.borrow_mut().insert(id.index(), func);
    }

    /// [`Self::define`] of the code of a value of the type of function
    /// `ty`, without a frame.
    fn define_value(&self, id: FuncId, ty: FuncTypeId, body: Vec<Stmt>) {
        let func = Func {
            value: Some(ty),
            ..self.func_of_sig(id, false, &[], body)
        };
        self.lowered.borrow_mut().insert(id.index(), func);
    }

    /// Makes `id`, a function of [`Self::reserve`], call `target` in its
    /// place.
    fn alias(&self, id: FuncId, target: FuncId) {
        let func = Func {
            alias: Some(target),
            ..self.func_of_sig(id, false, &[], Vec::new())
        };
        self.lowered.borrow_mut().insert(id.index(), func);
    }

    /// The function `id` with `body`, whose variables are its parameters,
    /// from its signature, and a local of each rep of `locals`.
    fn func_of_sig(&self, id: FuncId, frame: bool, locals: &[Rep], body: Vec<Stmt>) -> Func {
        let sig = self.funcs.borrow().get(id).clone();
        let vars = sig
            .params
            .iter()
            .chain(locals)
            .map(|rep| Var {
                rep: *rep,
                flagged: false,
                ordered: false,
                cell: false,
            })
            .collect();
        Func::new(frame, sig.params.len(), vars, sig.ret, body)
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
            Shape::Instance(class, _)
                if Some(class.file) == self.intrinsics && class.name == "Hash" =>
            {
                Some(Rep::Hash)
            }
            Shape::Instance(class, args)
                if Some(class.file) == self.intrinsics && class.name == "Array" =>
            {
                let [item] = <[Shape; 1]>::try_from(args).ok()?;
                Some(Rep::Array(self.array_of(self.type_arg(item)?)))
            }
            Shape::Instance(class, args)
                if let Some(unit) = self.prelude.iter().find(|unit| unit.file == class.file) =>
            {
                self.prelude_class(unit, &class.name, args).map(Rep::Object)
            }
            // CPython raises `TypeError` for a key without a `hash` when
            // it runs, and the compiler refuses the type.
            Shape::Dict(key, value) => {
                let unit = self.prelude_unit("dict")?;
                if !self.hashable(&key) {
                    return None;
                }
                self.prelude_class(unit, "Dict", vec![*key, *value])
                    .map(Rep::Object)
            }
            Shape::Set(item) => {
                let unit = self.prelude_unit("set")?;
                if !self.hashable(&item) {
                    return None;
                }
                self.prelude_class(unit, "Set", vec![*item])
                    .map(Rep::Object)
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
                let unit = self.prelude_unit("list")?;
                self.prelude_class(unit, "List", vec![*item])
                    .map(Rep::Object)
            }
            Shape::Callable(params, ret) => {
                let params = params
                    .into_iter()
                    .map(|param| self.rep_of(Some(param)))
                    .collect::<Option<Vec<_>>>()?;
                let ret = match *ret {
                    Shape::None => Rep::None,
                    ret => self.rep_of(Some(ret))?,
                };
                Some(Rep::Func(self.func_type(params, ret)))
            }
            Shape::Function(def) if let Some(ty) = self.inner_defs.borrow().get(&def) => {
                Some(Rep::Func(*ty))
            }
            Shape::Function(def) => {
                let funcs = self.funcs.borrow();
                let sig = funcs.get(*funcs.ids.get(&def)?);
                let (params, ret) = (sig.params.clone(), sig.ret);
                drop(funcs);
                Some(Rep::Func(self.func_type(params, ret)))
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

    /// The type of a function with parameters of `params` and a result of
    /// `ret`, which the first call for it creates.
    fn func_type(&self, params: Vec<Rep>, ret: Rep) -> FuncTypeId {
        let mut types = self.func_types.borrow_mut();
        let ty = FuncType { params, ret };
        if let Some(id) = types.ids.get(&ty) {
            return *id;
        }
        let id = FuncTypeId::new(types.list.len());
        types.list.push(ty.clone());
        types.ids.insert(ty, id);
        id
    }

    /// The parameters and the result of the type of function `ty`.
    fn func_type_of(&self, ty: FuncTypeId) -> FuncType {
        self.func_types
            .borrow()
            .list
            .get(ty.index())
            .expect("a type of function of the program")
            .clone()
    }

    /// The type of an array whose items have the type `arg`, which the
    /// first call for it creates.
    fn array_of(&self, arg: TypeArg) -> ArrayId {
        let mut arrays = self.arrays.borrow_mut();
        if let Some(array) = arrays.ids.get(&arg) {
            return *array;
        }
        let array = ArrayId::new(arrays.args.len());
        arrays.args.push(arg.clone());
        arrays.ids.insert(arg, array);
        array
    }

    /// What a type variable stands for when a generic class of the prelude
    /// takes `shape` for it. A type variable of the copy that the lowering
    /// is in stands for what it stands for there.
    fn type_arg(&self, shape: Shape) -> Option<TypeArg> {
        if let Shape::TypeVar(name) = &shape {
            let types = self.types.borrow();
            return types
                .iter()
                .find(|(var, _)| var == name)
                .map(|(_, arg)| arg.clone());
        }
        let rep = self.rep_of(Some(shape.clone()))?;
        let classes = match rep {
            Rep::Uniform => self.uniform_eq_classes(Some(shape)),
            _ => Vec::new(),
        };
        Some(TypeArg { rep, classes })
    }

    /// The type of each item of an array of the type `array`.
    fn array_arg(&self, array: ArrayId) -> TypeArg {
        self.arrays
            .borrow()
            .args
            .get(array.index())
            .expect("a type of array of the program")
            .clone()
    }

    /// Returns `true` if a uniform value may hold a value of `rep`, `false`
    /// otherwise. The dispatch of a uniform value knows no list, no
    /// function and no array yet.
    fn in_uniform(&self, rep: Rep) -> bool {
        self.list_class(rep).is_none() && rep.func().is_none() && !matches!(rep, Rep::Array(_))
    }

    /// The copy of the class `List` of the prelude whose objects `rep`
    /// holds, which may hold `None` too, and `None` for a rep of another
    /// type.
    pub(super) fn list_class(&self, rep: Rep) -> Option<ClassId> {
        let class = rep.class()?;
        let unit = self.prelude_unit("list")?;
        let is_list = self.copy_file(class) == Some(unit.file)
            && self.classes.borrow().get(class).def.name.as_str() == "List";
        is_list.then_some(class)
    }

    /// The rep of each item of a list of `rep`, which may hold `None` too,
    /// and `None` for a rep of another type.
    pub(super) fn list_item_of(&self, rep: Rep) -> Option<Rep> {
        let args = self.copy_args(self.list_class(rep)?)?;
        Some(args.first().expect("the type of the items").rep)
    }

    /// The rep of each item of an array of the type `array`.
    fn array_item_rep(&self, array: ArrayId) -> Rep {
        self.arrays
            .borrow()
            .args
            .get(array.index())
            .expect("a type of array of the program")
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
                None => Func::new(false, 0, Vec::new(), Rep::None, Vec::new()),
            });
        }
        Ok(())
    }

    /// Makes the function that gives the `repr` of a `str` and of an object
    /// of each class in `classes`, which the `repr` of a uniform value
    /// calls, and returns the one of the `str`.
    fn uniform_reprs(&self, classes: std::ops::Range<usize>) -> Result<Option<FuncId>, Error> {
        for i in classes {
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
            (Rep::Uniform, _) => self.in_uniform(value),
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
        } else if pair.iter().all(|rep| self.in_uniform(*rep)) {
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
        Rep::Hash | Rep::Array(_) => Err(unsupported(node, "a variable self of this type")),
        Rep::Func(_) => Err(unsupported(node, "a variable self of a function")),
        Rep::Int | Rep::Float | Rep::Bool | Rep::Str | Rep::None => Ok(()),
    }
}

/// Checks that `parameters` has no positional-only parameter, no `*args`
/// and no `**kwargs`.
fn check_params(parameters: &ast::Parameters) -> Result<(), Error> {
    if !parameters.posonlyargs.is_empty()
        || parameters.vararg.is_some()
        || parameters.kwarg.is_some()
    {
        return Err(unsupported(parameters, "these parameters"));
    }
    Ok(())
}

/// The read of a variable that [`Lowering::hold`] gives.
fn read((place, rep): (Place, Rep)) -> Expr {
    Expr::Load {
        place,
        rep,
        check: None,
    }
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported(Unsupported {
        range: node.range(),
        what: what.to_owned(),
        in_prelude: false,
    })
}

/// Each name of `target`, the target of a `for` over `iter`, with the type
/// of its values, when `iter` is a `dict`, its `keys()`, its `values()`,
/// or its `items()` with two names as the target, or a `set`. `None` for
/// another `for`.
fn dict_loop<'e>(
    model: &SemanticModel<'_>,
    target: &'e ast::Expr,
    iter: &ast::Expr,
) -> Option<Vec<(&'e ast::Expr, Shape)>> {
    let (value, part) = dict_part(iter);
    let (key, value) = match expr_shape(model, value)? {
        Shape::Dict(key, value) => (key, value),
        Shape::Set(item) if part == "keys" => return Some(vec![(target, *item)]),
        _ => return None,
    };
    match (part, target) {
        ("keys", target) => Some(vec![(target, *key)]),
        ("values", target) => Some(vec![(target, *value)]),
        ("items", ast::Expr::Tuple(tuple)) => match &tuple.elts[..] {
            [k, v] => Some(vec![(k, *key), (v, *value)]),
            _ => None,
        },
        _ => None,
    }
}

/// The value of `iter`, the iterable of a `for`, that may be a `dict`, and
/// what the loop takes of it: `keys` for the value itself, and `keys`,
/// `values` or `items` for a call of the method of the name without
/// arguments.
fn dict_part(iter: &ast::Expr) -> (&ast::Expr, &str) {
    match iter {
        ast::Expr::Call(call)
            if call.arguments.is_empty()
                && let ast::Expr::Attribute(attr) = &*call.func
                && matches!(attr.attr.as_str(), "keys" | "values" | "items") =>
        {
            (&*attr.value, attr.attr.as_str())
        }
        value => (value, "keys"),
    }
}
