//! From the syntax tree and the types of the checker to the IR.

mod call;
mod expr;
mod flow;
mod name_error;
mod stmt;
mod vars;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_python_ast as ast;
use ruff_text_size::{Ranged, TextRange};
use ty_python_semantic::types::lowering::{Shape, expr_shape, function_info};
use ty_python_semantic::{Db, SemanticModel};

use crate::Error;
use crate::cpython::traceback::{self, FileLines};
use crate::ir::{Func, FuncId, Program, Rep, Site, SiteFrame, Var, VarId};
use flow::{Assigned, Bindings, always_returns};
use name_error::unbound_local;

pub(crate) fn lower(db: &dyn Db, file: File, path: &str) -> Result<Program, Error> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let source = source_text(db, file);
    let lowering = Lowering {
        model: &model,
        source: source.as_str(),
        file: FileLines::new(source.as_str()),
        path,
        sites: RefCell::new(Vec::new()),
        vars: RefCell::new(Vars::default()),
        funcs: RefCell::new(Funcs::default()),
        scope: RefCell::new(None),
        lines: RefCell::new(HashMap::new()),
    };
    let suite = module.suite().as_slice();
    lowering.declare_vars(&Bindings::of(suite))?;
    let body = lowering.body(suite)?;
    let count = lowering.funcs.borrow().sigs.len();
    let mut funcs: Vec<Func> = (0..count)
        .map(|i| lowering.func(FuncId::new(i)))
        .collect::<Result<_, _>>()?;
    // The lowering of a later function may order an earlier one.
    for (func, ordered) in funcs.iter_mut().zip(&lowering.funcs.borrow().ordered) {
        func.ordered = *ordered;
    }
    Ok(Program {
        body,
        vars: lowering.vars.into_inner().list,
        funcs,
        sites: lowering.sites.into_inner(),
    })
}

struct Lowering<'a, 'db> {
    model: &'a SemanticModel<'db>,
    source: &'a str,
    /// The lines of `source`, for the frames of the sites.
    file: FileLines<'a>,
    /// The path of the file in a traceback.
    path: &'a str,
    sites: RefCell<Vec<SiteFrame>>,
    /// The variables of the module.
    vars: RefCell<Vars>,
    funcs: RefCell<Funcs<'a>>,
    /// The function whose body the lowering is in, and `None` in the code
    /// of the module.
    scope: RefCell<Option<Scope>>,
    /// The line of the frame of each line of each function, and of the
    /// module, by the name of the function and the number of the line.
    lines: RefCell<HashMap<(String, usize), u32>>,
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
        let id = VarId::new(self.list.len());
        self.ids.insert(name.to_owned(), id);
        self.list.push(Var {
            rep,
            flagged: false,
            ordered: false,
        });
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

#[derive(Default)]
struct Funcs<'a> {
    ids: HashMap<String, FuncId>,
    sigs: Vec<Sig<'a>>,
    /// Whether the program asks which globals got a value first, by the id
    /// of the function.
    ordered: Vec<bool>,
}

impl<'a> Funcs<'a> {
    fn get(&self, id: FuncId) -> &Sig<'a> {
        self.sigs.get(id.index()).expect("a function of the module")
    }
}

/// What a call needs to know of a function before its body is lowered.
#[derive(Clone)]
struct Sig<'a> {
    def: &'a ast::StmtFunctionDef,
    params: Vec<Rep>,
    ret: Rep,
}

struct Scope {
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
}

impl Scope {
    /// Returns `true` if `name` is a local of the function, `false` when
    /// the function reads it from the module.
    fn is_local(&self, name: &str) -> bool {
        self.locals.contains(name)
    }
}

impl<'a> Lowering<'a, '_> {
    /// Gives each variable that the code assigns the rep of its values,
    /// which the checker infers, and declares each function of the module.
    /// A read or a call may come before the assignment, as in a loop, so
    /// every variable has a rep before the lowering of the code starts.
    fn declare_vars(&self, bindings: &Bindings<'a>) -> Result<(), Error> {
        // The lowering of a function rejects a `def` in its body.
        if self.scope.borrow().is_none() {
            for def in &bindings.defs {
                self.declare_func(def)?;
                self.declare_globals(def)?;
            }
        }
        self.declare_assigned(&bindings.assignments, |name| {
            self.scope
                .borrow()
                .as_ref()
                .is_none_or(|scope| scope.is_local(name))
        })
    }

    /// Declares in the module each global that `def` assigns, since the
    /// code of the module may read it.
    fn declare_globals(&self, def: &'a ast::StmtFunctionDef) -> Result<(), Error> {
        let bindings = Bindings::of(&def.body);
        self.declare_assigned(&bindings.assignments, |name| {
            bindings.globals.contains(name)
        })
    }

    /// Declares each variable of `assigned` whose name `keep` accepts, in
    /// the scope that the lowering is in.
    fn declare_assigned(
        &self,
        assigned: &[Assigned<'a>],
        keep: impl Fn(&str) -> bool,
    ) -> Result<(), Error> {
        for (targets, value) in assigned {
            // The lowering of the assignment rejects a value without a rep.
            let Some(rep) = shape_rep(expr_shape(self.model, value)) else {
                continue;
            };
            for target in *targets {
                if let ast::Expr::Name(name) = target
                    && keep(name.id.as_str())
                {
                    self.declare_var(name.id.as_str(), rep, value)?;
                }
            }
        }
        Ok(())
    }

    fn declare_var(&self, name: &str, rep: Rep, value: &ast::Expr) -> Result<(), Error> {
        let mut scope = self.scope.borrow_mut();
        let mut module = self.vars.borrow_mut();
        let vars = match scope.as_mut() {
            Some(scope) => &mut scope.vars,
            None => {
                if self.funcs.borrow().ids.contains_key(name) {
                    return Err(unsupported(value, "a name of a function and of a variable"));
                }
                &mut *module
            }
        };
        match vars.find(name) {
            Some((_, declared)) if declared != rep => {
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
        if def.is_async || !def.decorator_list.is_empty() || def.type_params.is_some() {
            return Err(unsupported(def, "this def"));
        }
        let parameters = &def.parameters;
        if !parameters.posonlyargs.is_empty()
            || parameters.vararg.is_some()
            || !parameters.kwonlyargs.is_empty()
            || parameters.kwarg.is_some()
        {
            return Err(unsupported(&**parameters, "these parameters"));
        }
        if let Some(param) = parameters.args.iter().find(|p| p.default.is_some()) {
            return Err(unsupported(param, "a parameter with a default"));
        }
        if let Some(param) = parameters.args.get(MAX_PARAMS) {
            return Err(unsupported(param, "a def with more than 1000 parameters"));
        }
        let info = function_info(self.model, def).ok_or_else(|| unsupported(def, "this def"))?;
        let params = parameters
            .args
            .iter()
            .zip(info.params)
            .map(|(param, info)| {
                shape_rep(Some(info.shape))
                    .ok_or_else(|| unsupported(param, "a parameter of this type"))
            })
            .collect::<Result<_, _>>()?;
        let ret = match info.ret {
            Shape::None => Rep::None,
            shape => shape_rep(Some(shape)).ok_or_else(|| match &def.returns {
                Some(returns) => unsupported(&**returns, "a result of this type"),
                None => unsupported(&def.name, "a result of this type"),
            })?,
        };
        let name = def.name.as_str();
        if self.vars.borrow().ids.contains_key(name) {
            return Err(unsupported(def, "a name of a function and of a variable"));
        }
        let mut funcs = self.funcs.borrow_mut();
        if funcs.ids.contains_key(name) {
            return Err(unsupported(def, "two functions of the same name"));
        }
        let id = FuncId::new(funcs.sigs.len());
        funcs.ids.insert(name.to_owned(), id);
        funcs.sigs.push(Sig { def, params, ret });
        funcs.ordered.push(false);
        Ok(())
    }

    /// The body of the function, lowered in its own scope.
    fn func(&self, id: FuncId) -> Result<Func, Error> {
        let sig = self.funcs.borrow().get(id).clone();
        let def = sig.def;
        let mut vars = Vars::default();
        let mut locals = HashSet::new();
        let mut order = Vec::new();
        for (param, rep) in def.parameters.args.iter().zip(&sig.params) {
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
            name: def.name.to_string(),
            ret: sig.ret,
            locals,
            order,
            vars,
        });
        self.declare_vars(&bindings)?;
        let body = self.body(&def.body)?;
        if sig.ret != Rep::None && !always_returns(&def.body) {
            return Err(unsupported(
                &def.name,
                "a function that may end without a return",
            ));
        }
        let scope = self.scope.take().expect("the scope of the function");
        Ok(Func {
            ordered: false,
            params: u32::try_from(sig.params.len()).expect("fewer than 2^32 parameters"),
            vars: scope.vars.list,
            ret: sig.ret,
            body,
        })
    }

    /// A new site for the instruction at `range`, in the code that the
    /// lowering is in.
    fn site(&self, range: TextRange) -> Site {
        let name = self
            .scope
            .borrow()
            .as_ref()
            .map_or_else(|| "<module>".to_owned(), |scope| scope.name.clone());
        let number = traceback::line(&self.file, range);
        let text = traceback::frame(self.path, &name, &self.file, range);
        let mut lines = self.lines.borrow_mut();
        let count = u32::try_from(lines.len()).expect("fewer than 2^32 lines");
        let line = *lines.entry((name, number)).or_insert(count);
        let mut sites = self.sites.borrow_mut();
        sites.push(SiteFrame { text, line });
        Site::new(sites.len() - 1)
    }
}

/// The most parameters that a wasm engine accepts in a function.
const MAX_PARAMS: usize = 1000;

/// The rep of the values of `shape`, and `None` when the compiler has no rep
/// for them.
fn shape_rep(shape: Option<Shape>) -> Option<Rep> {
    match shape? {
        Shape::Int | Shape::IntLit(_) => Some(Rep::Int),
        Shape::Bool | Shape::BoolLit(_) => Some(Rep::Bool),
        Shape::Str | Shape::StrLit(_) => Some(Rep::Str),
        _ => None,
    }
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported {
        range: node.range(),
        what: what.to_owned(),
    }
}
