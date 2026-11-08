//! From the syntax tree and the types of the checker to the IR.

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::source_text;
use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_text_size::{Ranged, TextRange};
use ty_module_resolver::{KnownModule, file_to_module};
use ty_python_semantic::types::lowering::{
    Shape, expr_shape, function_info, is_definitely_bound, is_implicit_global,
};
use ty_python_semantic::{Db, SemanticModel};

use crate::Error;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::cpython::{names, suggest, traceback};
use crate::ir::{
    ArithOp, Binding, Check, CmpOp, Expr, Func, FuncId, Group, Limit, LogicOp, NameError, Place,
    Program, Rep, Site, SiteFrame, Stmt, Undefined, Var, VarId,
};

/// The range of an `int`, which has to fit in 31 bits.
const INT_MIN: i64 = -(1 << 30);
const INT_MAX: i64 = (1 << 30) - 1;

pub(crate) fn lower(db: &dyn Db, file: File, path: &str) -> Result<Program, Error> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db));
    let module = parsed.load(db);
    let model = SemanticModel::new(db, program_file);
    let source = source_text(db, file);
    let lowering = Lowering {
        model: &model,
        source: source.as_str(),
        path,
        sites: RefCell::new(Vec::new()),
        vars: RefCell::new(Vars::default()),
        funcs: RefCell::new(Funcs::default()),
        scope: RefCell::new(None),
        lines: RefCell::new(HashMap::new()),
    };
    let suite = module.suite().as_slice();
    lowering.declare_vars(suite)?;
    let body = lowering.body(suite)?;
    let count = lowering.funcs.borrow().sigs.len();
    let mut funcs: Vec<Func> = (0..count)
        .map(|i| lowering.func(FuncId(u32::try_from(i).expect("fewer than 2^32 functions"))))
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
        self.list
            .get(id.0 as usize)
            .expect("a variable of the scope")
    }

    fn get_mut(&mut self, id: VarId) -> &mut Var {
        self.list
            .get_mut(id.0 as usize)
            .expect("a variable of the scope")
    }

    fn add(&mut self, name: &str, rep: Rep) {
        let id = VarId(u32::try_from(self.list.len()).expect("fewer than 2^32 variables"));
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
        names.sort_by_key(|(id, _)| id.0);
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
        self.sigs
            .get(id.0 as usize)
            .expect("a function of the module")
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
    /// The names of the `global` statements of the function.
    globals: HashSet<String>,
    /// The locals in the order of the frame of CPython.
    order: Vec<String>,
    vars: Vars,
}

impl<'a> Lowering<'a, '_> {
    /// Gives each variable that the code assigns the rep of its values,
    /// which the checker infers, and declares each function of the module.
    /// A read or a call may come before the assignment, as in a loop, so
    /// every variable has a rep before the lowering of the code starts.
    fn declare_vars(&self, body: &'a [ast::Stmt]) -> Result<(), Error> {
        let mut assigned = Vec::new();
        let mut defs = Vec::new();
        assignments(body, &mut assigned, &mut defs);
        // The lowering of a function rejects a `def` in its body.
        if self.scope.borrow().is_none() {
            for def in defs {
                self.declare_func(def)?;
                self.declare_globals(def)?;
            }
        }
        let globals = self
            .scope
            .borrow()
            .as_ref()
            .map(|scope| scope.globals.clone())
            .unwrap_or_default();
        self.declare_assigned(&assigned, |name| !globals.contains(name))
    }

    /// Declares in the module each global that `def` assigns, since the
    /// code of the module may read it.
    fn declare_globals(&self, def: &'a ast::StmtFunctionDef) -> Result<(), Error> {
        let mut globals = HashSet::new();
        global_names(&def.body, &mut globals);
        let mut assigned = Vec::new();
        assignments(&def.body, &mut assigned, &mut Vec::new());
        self.declare_assigned(&assigned, |name| globals.contains(name))
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
        let id = FuncId(u32::try_from(funcs.sigs.len()).expect("fewer than 2^32 functions"));
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
        local_names(&def.body, &mut locals);
        let mut globals = HashSet::new();
        global_names(&def.body, &mut globals);
        locals.retain(|name| !globals.contains(name));
        code_order(&def.body, &mut order);
        let mut seen = HashSet::new();
        let order = order
            .into_iter()
            .filter(|name| locals.contains(*name) && seen.insert(*name))
            .map(str::to_owned)
            .collect();
        *self.scope.borrow_mut() = Some(Scope {
            name: def.name.to_string(),
            ret: sig.ret,
            locals,
            globals,
            order,
            vars,
        });
        self.declare_vars(&def.body)?;
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

    fn body(&self, body: &[ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            lowered.extend(self.stmt(stmt)?);
        }
        Ok(lowered)
    }

    fn stmt(&self, stmt: &ast::Stmt) -> Result<Option<Stmt>, Error> {
        match stmt {
            ast::Stmt::Expr(e) => Ok(Some(Stmt::Expr(self.expr(&e.value)?))),
            ast::Stmt::Assign(a) => {
                let value = self.expr(&a.value)?;
                let targets = a
                    .targets
                    .iter()
                    .map(|target| self.target(target, &value))
                    .collect::<Result<_, _>>()?;
                Ok(Some(Stmt::Assign { targets, value }))
            }
            // An annotation alone binds nothing.
            ast::Stmt::AnnAssign(ast::StmtAnnAssign { value: None, .. }) => Ok(None),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                let value = self.expr(value)?;
                let target = self.target(target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::AugAssign(a) => {
                let ast::Expr::Name(name) = &*a.target else {
                    return Err(unsupported(&*a.target, "this target"));
                };
                let left = self.load(name)?;
                let right = self.expr(&a.value)?;
                let value = self.binary(a, a.op, (left, name), (right, &*a.value))?;
                let target = self.target(&a.target, &value)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::If(i) => {
                let mut else_ = Vec::new();
                // Each `elif` is an `if` in the `else` of the clause before it.
                for clause in i.elif_else_clauses.iter().rev() {
                    let body = self.body(&clause.body)?;
                    else_ = match &clause.test {
                        Some(test) => vec![Stmt::If {
                            cond: self.truth_of(test)?,
                            then: body,
                            else_,
                        }],
                        None => body,
                    };
                }
                Ok(Some(Stmt::If {
                    cond: self.truth_of(&i.test)?,
                    then: self.body(&i.body)?,
                    else_,
                }))
            }
            ast::Stmt::While(w) => Ok(Some(Stmt::While {
                cond: self.truth_of(&w.test)?,
                body: self.body(&w.body)?,
                else_: self.body(&w.orelse)?,
            })),
            ast::Stmt::Break(_) => Ok(Some(Stmt::Break)),
            ast::Stmt::Continue(_) => Ok(Some(Stmt::Continue)),
            // `declare_vars` and `func` read the names.
            ast::Stmt::Pass(_) | ast::Stmt::Global(_) => Ok(None),
            ast::Stmt::FunctionDef(def) => {
                if self.scope.borrow().is_some() {
                    return Err(unsupported(def, "a def in a function"));
                }
                let id = *self
                    .funcs
                    .borrow()
                    .ids
                    .get(def.name.as_str())
                    .expect("a declared function");
                Ok(Some(Stmt::Def(id)))
            }
            ast::Stmt::Return(r) => self.return_(r).map(Some),
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    fn return_(&self, r: &ast::StmtReturn) -> Result<Stmt, Error> {
        let Some(ret) = self.scope.borrow().as_ref().map(|scope| scope.ret) else {
            return Err(unsupported(r, "a return outside a function"));
        };
        match r.value.as_deref() {
            None | Some(ast::Expr::NoneLiteral(_)) if ret == Rep::None => Ok(Stmt::Return(None)),
            None | Some(ast::Expr::NoneLiteral(_)) => Err(unsupported(r, "a return of None")),
            Some(value) => {
                let lowered = self.expr(value)?;
                if lowered.rep() != ret {
                    return Err(unsupported(value, "a return of this type"));
                }
                Ok(Stmt::Return(Some(lowered)))
            }
        }
    }

    /// The truth of `expr` as a `bool`, where only the truth matters, as
    /// in a condition. The truth of `a and b` is the truth of `a` and the
    /// truth of `b`, so `a` and `b` may have two types.
    fn truth_of(&self, expr: &ast::Expr) -> Result<Expr, Error> {
        match expr {
            ast::Expr::BoolOp(b) => self.bool_op(b, Self::truth_of),
            ast::Expr::UnaryOp(ast::ExprUnaryOp {
                op: ast::UnaryOp::Not,
                operand,
                ..
            }) => Ok(Expr::Not(Box::new(self.truth_of(operand)?))),
            _ => truth(self.expr(expr)?, expr),
        }
    }

    /// `and` or `or`, with each operand lowered by `operand`.
    fn bool_op(
        &self,
        b: &ast::ExprBoolOp,
        operand: impl Fn(&Self, &ast::Expr) -> Result<Expr, Error>,
    ) -> Result<Expr, Error> {
        let op = match b.op {
            ast::BoolOp::And => LogicOp::And,
            ast::BoolOp::Or => LogicOp::Or,
        };
        let (first, rest) = b
            .values
            .split_first()
            .expect("a bool operator has operands");
        let first = operand(self, first)?;
        let rep = first.rep();
        // `a and b and c` gives the value of `(a and b) and c`, and
        // evaluates the same operands.
        rest.iter().try_fold(first, |left, right| {
            let lowered = operand(self, right)?;
            if lowered.rep() != rep {
                return Err(unsupported(b, "operands of two types"));
            }
            Ok(Expr::Logic {
                op,
                left: Box::new(left),
                right: Box::new(lowered),
            })
        })
    }

    /// The variable that `target` names, which has to hold the rep of
    /// `value`.
    fn target(&self, target: &ast::Expr, value: &Expr) -> Result<Place, Error> {
        let ast::Expr::Name(name) = target else {
            return Err(unsupported(target, "this target"));
        };
        let name = name.id.as_str();
        let found = match self.scope.borrow().as_ref() {
            Some(scope) if !scope.globals.contains(name) => scope
                .vars
                .find(name)
                .map(|(id, rep)| (Place::Local(id), rep)),
            _ => self
                .vars
                .borrow()
                .find(name)
                .map(|(id, rep)| (Place::Global(id), rep)),
        };
        match found {
            Some((place, rep)) if rep == value.rep() => Ok(place),
            Some(_) => Err(unsupported(target, "a variable with values of two types")),
            // The checker gives the value a type that no rep holds.
            None => Err(unsupported(target, "a variable of this type")),
        }
    }

    /// The value of the variable that `name` reads.
    fn load(&self, name: &ast::ExprName) -> Result<Expr, Error> {
        let id = name.id.as_str();
        let local = match self.scope.borrow().as_ref() {
            Some(scope) if scope.locals.contains(id) => Some(
                scope
                    .vars
                    .find(id)
                    .ok_or_else(|| unsupported(name, "a variable of this type"))?,
            ),
            _ => None,
        };
        if let Some((var, rep)) = local {
            let check = if is_definitely_bound(self.model, name) {
                None
            } else {
                if let Some(scope) = self.scope.borrow_mut().as_mut() {
                    scope.vars.get_mut(var).flagged = true;
                }
                Some(Check {
                    site: self.site(name.range()),
                    error: Undefined::Local(unbound_local(id)),
                })
            };
            return Ok(Expr::Load {
                place: Place::Local(var),
                rep,
                check,
            });
        }
        let global = self.vars.borrow().find(id);
        let Some((var, rep)) = global else {
            if self.funcs.borrow().ids.contains_key(id) {
                return Err(unsupported(name, "a function as a value"));
            }
            return Err(unsupported(name, "a name that the module does not assign"));
        };
        let check = self.global_check(name)?;
        if check.is_some() {
            self.vars.borrow_mut().get_mut(var).flagged = true;
        }
        Ok(Expr::Load {
            place: Place::Global(var),
            rep,
            check,
        })
    }

    /// The site where the read of the global `name` raises `NameError`, and
    /// `None` when the read always finds a value. A function may run before
    /// the module assigns the global, so every read in a function checks.
    fn global_check(&self, name: &ast::ExprName) -> Result<Option<Check>, Error> {
        if self.scope.borrow().is_none() && is_definitely_bound(self.model, name) {
            return Ok(None);
        }
        if is_implicit_global(self.model, name.id.as_str()) {
            // Without a value, the read takes the builtin of the name.
            return Err(unsupported(name, "a name that may be a builtin"));
        }
        Ok(Some(Check {
            site: self.site(name.range()),
            error: Undefined::Global(Box::new(self.name_error(name.id.as_str()))),
        }))
    }

    /// The `NameError` of `name` in the frame that the lowering is in, with
    /// the candidates of the suggestion of CPython. Each candidate that the
    /// program asks about gets a flag.
    fn name_error(&self, name: &str) -> NameError {
        let mut scope = self.scope.borrow_mut();
        let mut module = self.vars.borrow_mut();
        let mut funcs = self.funcs.borrow_mut();
        let locals: Vec<(VarId, String)> = scope.as_ref().map_or_else(Vec::new, |scope| {
            scope
                .order
                .iter()
                .filter_map(|local| Some((scope.vars.find(local)?.0, local.clone())))
                .collect()
        });
        let globals: Vec<(Binding, String)> = module
            .names()
            .into_iter()
            .map(|(id, global)| (Binding::Global(id), global.to_owned()))
            .chain(
                funcs
                    .ids
                    .iter()
                    .map(|(func, id)| (Binding::Func(*id), func.clone())),
            )
            .collect();

        // CPython lists the locals, the globals and the builtins, and the
        // frame of the module lists its globals as its locals too.
        let (factor, params) = match scope.as_ref() {
            Some(scope) => {
                let id = funcs
                    .ids
                    .get(&scope.name)
                    .expect("the function of the scope");
                (1, funcs.get(*id).params.len())
            }
            None => (2, 0),
        };
        let most = locals.len()
            + factor * (names::MODULE_GLOBALS.len() + globals.len())
            + names::BUILTINS.len();
        let limit = (most >= suggest::MAX_CANDIDATE_ITEMS).then(|| Limit {
            max: count(suggest::MAX_CANDIDATE_ITEMS),
            always: count(params + factor * names::MODULE_GLOBALS.len() + names::BUILTINS.len()),
            globals: count(factor),
            locals: locals.iter().skip(params).map(|(id, _)| *id).collect(),
        });

        let mut by_distance: BTreeMap<usize, Group> = BTreeMap::new();
        for (id, local) in &locals {
            if let Some(group) = group_of(&mut by_distance, name, local) {
                group.locals.push((*id, local.clone()));
            }
        }
        // The globals of the module come before the globals of the program.
        for global in names::MODULE_GLOBALS {
            if let Some(group) = group_of(&mut by_distance, name, global) {
                group.always.get_or_insert_with(|| (*global).to_owned());
            }
        }
        for (binding, global) in &globals {
            if let Some(group) = group_of(&mut by_distance, name, global)
                && group.always.is_none()
            {
                group.globals.push((*binding, global.clone()));
            }
        }
        for builtin in names::BUILTINS {
            if let Some(group) = group_of(&mut by_distance, name, builtin) {
                group.always.get_or_insert_with(|| (*builtin).to_owned());
            }
        }
        let mut groups = Vec::new();
        for group in by_distance.into_values() {
            let last = group.always.is_some();
            groups.push(group);
            if last {
                break;
            }
        }

        let self_attr = match scope.as_ref() {
            Some(scope) => scope
                .vars
                .find("self")
                .filter(|_| scope.locals.contains("self"))
                .filter(|(_, rep)| has_attribute(Some(*rep), name))
                .map(|(id, _)| Binding::Local(id)),
            None => match (module.find("self"), funcs.ids.get("self")) {
                (Some((id, rep)), _) if has_attribute(Some(rep), name) => Some(Binding::Global(id)),
                (_, Some(id)) if has_attribute(None, name) => Some(Binding::Func(*id)),
                _ => None,
            },
        };

        let mut flag = |binding: Binding, ordered: bool| match binding {
            Binding::Local(id) => {
                if let Some(scope) = scope.as_mut() {
                    scope.vars.get_mut(id).flagged = true;
                }
            }
            Binding::Global(id) => {
                let var = module.get_mut(id);
                var.flagged = true;
                var.ordered |= ordered;
            }
            Binding::Func(id) => {
                *funcs
                    .ordered
                    .get_mut(id.0 as usize)
                    .expect("a function of the module") |= ordered;
            }
        };
        // The number of globals with a value is the number of ordered
        // globals with a value.
        let all_ordered = limit.is_some();
        if all_ordered {
            for (binding, _) in &globals {
                flag(*binding, true);
            }
        }
        for local in limit.iter().flat_map(|limit| &limit.locals) {
            flag(Binding::Local(*local), false);
        }
        for group in &groups {
            for (id, _) in &group.locals {
                flag(Binding::Local(*id), false);
            }
            for (binding, _) in &group.globals {
                flag(*binding, group.globals.len() > 1);
            }
        }
        if let Some(binding) = self_attr {
            flag(binding, false);
        }
        let base = format!("NameError: name '{name}' is not defined");
        let (import, or_import) = if names::STDLIB_MODULES.binary_search(&name).is_ok() {
            (
                format!(". Did you forget to import '{name}'?"),
                format!(" Or did you forget to import '{name}'?"),
            )
        } else {
            (String::new(), String::new())
        };
        NameError {
            message: format!("{base}{import}"),
            suggested: (format!("{base}. Did you mean: '"), format!("'?{or_import}")),
            self_attr: self_attr.map(|binding| (binding, format!("self.{name}"))),
            limit,
            groups,
        }
    }

    /// A call of a function of the module, with positional arguments.
    fn call(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ast::Expr::Name(name) = &*call.func else {
            return Err(unsupported(call, "this call"));
        };
        let callee = name.id.as_str();
        if self
            .scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.locals.contains(callee))
        {
            return Err(unsupported(call, "this call"));
        }
        let Some(func) = self.funcs.borrow().ids.get(callee).copied() else {
            return Err(unsupported(call, "this call"));
        };
        // The `def` is the one binding of the name in the module, so the call
        // finds the function, or raises `NameError` before the `def` runs.
        let sig = self.funcs.borrow().get(func).clone();
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "a call with keyword arguments"));
        }
        if call.arguments.args.len() != sig.params.len() {
            return Err(unsupported(call, "a call with this number of arguments"));
        }
        let check = self.global_check(name)?;
        let args = call
            .arguments
            .args
            .iter()
            .zip(&sig.params)
            .map(|(arg, rep)| {
                let lowered = self.expr(arg)?;
                if lowered.rep() != *rep {
                    return Err(unsupported(arg, "an argument of this type"));
                }
                Ok(lowered)
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Call {
            func,
            args,
            rep: sig.ret,
            site: self.site(call.range()),
            check,
        })
    }

    fn expr(&self, expr: &ast::Expr) -> Result<Expr, Error> {
        match expr {
            ast::Expr::StringLiteral(s) => {
                let value = s.value.to_str();
                // The parser reads the escape of a lone surrogate, as in
                // "\ud800", as U+FFFD. Python keeps the surrogate, and printing
                // it raises UnicodeEncodeError.
                let source = self
                    .source
                    .get(std::ops::Range::<usize>::from(expr.range()));
                if value.contains('\u{fffd}') && source.is_none_or(|text| text.contains('\\')) {
                    return Err(unsupported(
                        expr,
                        "a str with an escape that may be a surrogate",
                    ));
                }
                Ok(Expr::Str(value.to_owned()))
            }
            ast::Expr::NumberLiteral(n) => match &n.value {
                ast::Number::Int(i) => int_literal(expr, i.as_i64()),
                _ => Err(unsupported(expr, "this number")),
            },
            ast::Expr::BooleanLiteral(b) => Ok(Expr::Bool(b.value)),
            ast::Expr::Name(name) => self.load(name),
            ast::Expr::UnaryOp(u) => self.unary(u),
            ast::Expr::BinOp(b) => {
                let left = self.expr(&b.left)?;
                let right = self.expr(&b.right)?;
                self.binary(b, b.op, (left, &*b.left), (right, &*b.right))
            }
            ast::Expr::BoolOp(b) => self.bool_op(b, Self::logic_operand),
            ast::Expr::Compare(c) => self.compare(c),
            ast::Expr::Subscript(sub) => {
                let value = self.expr(&sub.value)?;
                if value.rep() != Rep::Str {
                    return Err(unsupported(&*sub.value, "a subscript of this type"));
                }
                if matches!(&*sub.slice, ast::Expr::Slice(_)) {
                    return Err(unsupported(&*sub.slice, "a slice"));
                }
                let index = self.int_operand(&sub.slice)?;
                Ok(Expr::Index {
                    value: Box::new(value),
                    index: Box::new(index),
                    site: self.site(sub.range()),
                })
            }
            ast::Expr::Call(call) if self.is_builtin(&call.func, "print") => self.print(call),
            ast::Expr::Call(call) if self.is_builtin(&call.func, "len") => self.len(call),
            ast::Expr::Call(call) => self.call(call),
            _ => Err(unsupported(expr, "this expression")),
        }
    }

    /// A chain of comparisons, where every operand is a `str`, or every
    /// operand is an `int` or a `bool`.
    fn compare(&self, c: &ast::ExprCompare) -> Result<Expr, Error> {
        let ops = c
            .ops
            .iter()
            .map(|op| match op {
                ast::CmpOp::Eq => Ok(CmpOp::Eq),
                ast::CmpOp::NotEq => Ok(CmpOp::Ne),
                ast::CmpOp::Lt => Ok(CmpOp::Lt),
                ast::CmpOp::LtE => Ok(CmpOp::Le),
                ast::CmpOp::Gt => Ok(CmpOp::Gt),
                ast::CmpOp::GtE => Ok(CmpOp::Ge),
                _ => Err(unsupported(c, "this comparison")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let operands = std::iter::once(c.first_operand())
            .chain(c.comparators())
            .map(|operand| Ok((self.expr(operand)?, operand)))
            .collect::<Result<Vec<_>, Error>>()?;
        let is_str = |(lowered, _): &(Expr, _)| lowered.rep() == Rep::Str;
        let mut operands = if !operands.iter().any(is_str) {
            operands
                .into_iter()
                .map(|(lowered, operand)| self.int_value(lowered, operand))
                .collect::<Result<Vec<_>, _>>()?
        } else if operands.iter().all(is_str) {
            operands.into_iter().map(|(lowered, _)| lowered).collect()
        } else {
            return Err(unsupported(c, "a comparison of two types"));
        }
        .into_iter();
        let first = operands.next().expect("a comparison has a first operand");
        Ok(Expr::Compare(
            Box::new(first),
            ops.into_iter().zip(operands).collect(),
        ))
    }

    /// The operator `op` of `node` on two lowered operands, each with the
    /// node it comes from.
    fn binary(
        &self,
        node: &impl Ranged,
        op: ast::Operator,
        (left, left_node): (Expr, &impl Ranged),
        (right, right_node): (Expr, &impl Ranged),
    ) -> Result<Expr, Error> {
        match (op, left.rep(), right.rep()) {
            (ast::Operator::Add, Rep::Str, Rep::Str) => {
                return Ok(Expr::Concat {
                    left: Box::new(left),
                    right: Box::new(right),
                    site: self.site(node.range()),
                });
            }
            (ast::Operator::Mult, Rep::Str, _) => {
                return Ok(Expr::Repeat {
                    left: Box::new(left),
                    right: Box::new(self.int_value(right, right_node)?),
                    site: self.site(node.range()),
                });
            }
            (ast::Operator::Mult, _, Rep::Str) => {
                return Ok(Expr::Repeat {
                    left: Box::new(self.int_value(left, left_node)?),
                    right: Box::new(right),
                    site: self.site(node.range()),
                });
            }
            (_, Rep::Str, _) | (_, _, Rep::Str) => return Err(unsupported(node, "this operator")),
            _ => {}
        }
        let op = arith_op(op).ok_or_else(|| unsupported(node, "this operator"))?;
        Ok(Expr::Arith {
            op,
            left: Box::new(self.int_value(left, left_node)?),
            right: Box::new(self.int_value(right, right_node)?),
            site: self.site(node.range()),
        })
    }

    fn unary(&self, u: &ast::ExprUnaryOp) -> Result<Expr, Error> {
        match u.op {
            // The literal alone may not fit, as in `-1073741824`.
            ast::UnaryOp::USub => match &*u.operand {
                ast::Expr::NumberLiteral(ast::ExprNumberLiteral {
                    value: ast::Number::Int(i),
                    ..
                }) => int_literal(u, i.as_i64().map(|i| -i)),
                operand => {
                    let operand = self.int_operand(operand)?;
                    Ok(Expr::Arith {
                        op: ArithOp::Sub,
                        left: Box::new(Expr::Int(0)),
                        right: Box::new(operand),
                        site: self.site(u.range()),
                    })
                }
            },
            ast::UnaryOp::UAdd => self.int_operand(&u.operand),
            ast::UnaryOp::Not => Ok(Expr::Not(Box::new(self.truth_of(&u.operand)?))),
            ast::UnaryOp::Invert => Err(unsupported(u, "this operator")),
        }
    }

    /// A new site for the instruction at `range`, in the code that the
    /// lowering is in.
    fn site(&self, range: TextRange) -> Site {
        let name = self
            .scope
            .borrow()
            .as_ref()
            .map_or_else(|| "<module>".to_owned(), |scope| scope.name.clone());
        let number = traceback::line(self.source, range);
        let text = traceback::frame(self.path, &name, self.source, range);
        let mut lines = self.lines.borrow_mut();
        let count = u32::try_from(lines.len()).expect("fewer than 2^32 lines");
        let line = *lines.entry((name, number)).or_insert(count);
        let mut sites = self.sites.borrow_mut();
        sites.push(SiteFrame { text, line });
        Site(u32::try_from(sites.len() - 1).expect("fewer than 2^32 sites"))
    }

    /// An operand of `and` or `or`, which has to have a truth.
    fn logic_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(operand)?;
        match lowered.rep() {
            Rep::None => Err(unsupported(operand, "the truth of None")),
            _ => Ok(lowered),
        }
    }

    /// The operand of an operator of `int`, which may be a `bool`.
    fn int_operand(&self, operand: &ast::Expr) -> Result<Expr, Error> {
        self.int_value(self.expr(operand)?, operand)
    }

    /// `lowered`, the value of `operand`, as an `int`.
    fn int_value(&self, lowered: Expr, operand: &impl Ranged) -> Result<Expr, Error> {
        match lowered.rep() {
            Rep::Int => Ok(lowered),
            Rep::Bool => Ok(Expr::BoolAsInt(Box::new(lowered))),
            Rep::Str | Rep::None => Err(unsupported(
                operand,
                "an operator on a value that is not an int",
            )),
        }
    }

    fn print(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        if !call.arguments.keywords.is_empty() {
            return Err(unsupported(call, "print with keyword arguments"));
        }
        let args = call
            .arguments
            .args
            .iter()
            .map(|arg| {
                let lowered = self.expr(arg)?;
                match lowered.rep() {
                    Rep::None => Err(unsupported(arg, "print of None")),
                    _ => Ok(lowered),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Expr::Print(args))
    }

    /// `len` of one `str`.
    fn len(&self, call: &ast::ExprCall) -> Result<Expr, Error> {
        let ([arg], []) = (&*call.arguments.args, &*call.arguments.keywords) else {
            return Err(unsupported(call, "this call of len"));
        };
        let operand = self.expr(arg)?;
        if operand.rep() != Rep::Str {
            return Err(unsupported(arg, "len of this type"));
        }
        Ok(Expr::Len {
            operand: Box::new(operand),
            site: self.site(call.range()),
        })
    }

    /// Returns `true` if `func` is the function `name` of the builtins,
    /// `false` otherwise.
    fn is_builtin(&self, func: &ast::Expr, name: &str) -> bool {
        matches!(
            expr_shape(self.model, func),
            Some(Shape::Function(def)) if def.name == name && is_builtins(self.model.db(), def.file)
        )
    }
}

/// Returns `true` if `file` is the `builtins` module, `false` otherwise.
fn is_builtins(db: &dyn Db, file: File) -> bool {
    let resolver_file = db.program_file(file).resolver_file(db);
    file_to_module(db, resolver_file).is_some_and(|m| m.is_known(db, KnownModule::Builtins))
}

fn arith_op(op: ast::Operator) -> Option<ArithOp> {
    match op {
        ast::Operator::Add => Some(ArithOp::Add),
        ast::Operator::Sub => Some(ArithOp::Sub),
        ast::Operator::Mult => Some(ArithOp::Mul),
        ast::Operator::FloorDiv => Some(ArithOp::FloorDiv),
        ast::Operator::Mod => Some(ArithOp::Mod),
        _ => None,
    }
}

/// Returns `true` if a value of `rep` has the attribute `name`, `false`
/// otherwise. A `rep` of `None` stands for a function.
fn has_attribute(rep: Option<Rep>, name: &str) -> bool {
    let attributes = match rep {
        Some(Rep::Int) => names::INT_ATTRIBUTES,
        Some(Rep::Bool) => names::BOOL_ATTRIBUTES,
        Some(Rep::Str) => names::STR_ATTRIBUTES,
        Some(Rep::None) => return false,
        None => names::FUNCTION_ATTRIBUTES,
    };
    attributes.binary_search(&name).is_ok()
}

/// The group of the distance from `name` to `candidate`, and `None` when
/// CPython would not suggest `candidate` for `name`.
fn group_of<'m>(
    groups: &'m mut BTreeMap<usize, Group>,
    name: &str,
    candidate: &str,
) -> Option<&'m mut Group> {
    if candidate == name {
        return None;
    }
    let max = suggest::max_distance(name, candidate);
    let distance = suggest::distance(name, candidate, max);
    (distance <= max).then(|| {
        groups.entry(distance).or_insert_with(|| Group {
            locals: Vec::new(),
            globals: Vec::new(),
            always: None,
        })
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).expect("fewer than 2^32 names")
}

/// Adds to `names` each name of `body` in the order of the code that
/// CPython makes for it, which gives the order of the locals of a frame.
/// The value of an assignment comes before its targets.
fn code_order<'a>(body: &'a [ast::Stmt], names: &mut Vec<&'a str>) {
    struct Names<'a, 'b>(&'b mut Vec<&'a str>);
    impl<'a> Visitor<'a> for Names<'a, '_> {
        fn visit_expr(&mut self, expr: &'a ast::Expr) {
            if let ast::Expr::Name(name) = expr {
                self.0.push(name.id.as_str());
            }
            walk_expr(self, expr);
        }
    }
    let visit = |expr: &'a ast::Expr, names: &mut Vec<&'a str>| {
        Names(names).visit_expr(expr);
    };
    for stmt in body {
        match stmt {
            ast::Stmt::Expr(e) => visit(&e.value, names),
            ast::Stmt::Assign(a) => {
                visit(&a.value, names);
                for target in &a.targets {
                    visit(target, names);
                }
            }
            ast::Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    visit(value, names);
                }
                visit(&a.target, names);
            }
            ast::Stmt::AugAssign(a) => {
                visit(&a.target, names);
                visit(&a.value, names);
            }
            ast::Stmt::If(i) => {
                visit(&i.test, names);
                code_order(&i.body, names);
                for clause in &i.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        visit(test, names);
                    }
                    code_order(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                visit(&w.test, names);
                code_order(&w.body, names);
                code_order(&w.orelse, names);
            }
            ast::Stmt::Return(r) => {
                if let Some(value) = &r.value {
                    visit(value, names);
                }
            }
            _ => {}
        }
    }
}

fn unbound_local(name: &str) -> String {
    format!(
        "UnboundLocalError: cannot access local variable '{name}' where it is not \
         associated with a value"
    )
}

/// The targets of an assignment, with the value.
type Assigned<'a> = (&'a [ast::Expr], &'a ast::Expr);

/// Adds to `assigned` each assignment of a value in `body`, and to `defs`
/// each `def`, out of the `if`s and the `while`s too.
fn assignments<'a>(
    body: &'a [ast::Stmt],
    assigned: &mut Vec<Assigned<'a>>,
    defs: &mut Vec<&'a ast::StmtFunctionDef>,
) {
    for stmt in body {
        match stmt {
            ast::Stmt::If(i) => {
                assignments(&i.body, assigned, defs);
                for clause in &i.elif_else_clauses {
                    assignments(&clause.body, assigned, defs);
                }
            }
            ast::Stmt::While(w) => {
                assignments(&w.body, assigned, defs);
                assignments(&w.orelse, assigned, defs);
            }
            ast::Stmt::FunctionDef(def) => defs.push(def),
            ast::Stmt::Assign(a) => assigned.push((&a.targets, &a.value)),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => assigned.push((std::slice::from_ref(target), value)),
            _ => {}
        }
    }
}

/// Adds to `names` each name of a `global` statement in `body`.
fn global_names(body: &[ast::Stmt], names: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            ast::Stmt::Global(g) => names.extend(g.names.iter().map(|name| name.id.to_string())),
            ast::Stmt::If(i) => {
                global_names(&i.body, names);
                for clause in &i.elif_else_clauses {
                    global_names(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                global_names(&w.body, names);
                global_names(&w.orelse, names);
            }
            _ => {}
        }
    }
}

/// Adds to `names` each name that `body` assigns. Any other way to bind a
/// name, as a `for` or a `:=`, makes the lowering of the function fail.
fn local_names(body: &[ast::Stmt], names: &mut HashSet<String>) {
    fn add(target: &ast::Expr, names: &mut HashSet<String>) {
        if let ast::Expr::Name(name) = target {
            names.insert(name.id.to_string());
        }
    }
    for stmt in body {
        match stmt {
            ast::Stmt::Assign(a) => {
                for target in &a.targets {
                    add(target, names);
                }
            }
            ast::Stmt::AnnAssign(a) => add(&a.target, names),
            ast::Stmt::AugAssign(a) => add(&a.target, names),
            ast::Stmt::If(i) => {
                local_names(&i.body, names);
                for clause in &i.elif_else_clauses {
                    local_names(&clause.body, names);
                }
            }
            ast::Stmt::While(w) => {
                local_names(&w.body, names);
                local_names(&w.orelse, names);
            }
            _ => {}
        }
    }
}

/// Returns `true` if `body` never reaches its end, `false` when it may.
/// The answer is `false` in a case this check cannot decide.
fn always_returns(body: &[ast::Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        ast::Stmt::Return(_) => true,
        ast::Stmt::If(i) => {
            always_returns(&i.body)
                && i.elif_else_clauses
                    .last()
                    .is_some_and(|clause| clause.test.is_none())
                && i.elif_else_clauses
                    .iter()
                    .all(|clause| always_returns(&clause.body))
        }
        ast::Stmt::While(w) => {
            matches!(&*w.test, ast::Expr::BooleanLiteral(b) if b.value) && !breaks(&w.body)
        }
        _ => false,
    })
}

/// Returns `true` if `body` has a `break` of the loop whose body it is,
/// `false` otherwise.
fn breaks(body: &[ast::Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        ast::Stmt::Break(_) => true,
        ast::Stmt::If(i) => {
            breaks(&i.body)
                || i.elif_else_clauses
                    .iter()
                    .any(|clause| breaks(&clause.body))
        }
        _ => false,
    })
}

/// The truth of `lowered`, the value of `node`, as a `bool`.
fn truth(lowered: Expr, node: &impl Ranged) -> Result<Expr, Error> {
    match lowered.rep() {
        Rep::Bool => Ok(lowered),
        Rep::Int | Rep::Str => Ok(Expr::Truth(Box::new(lowered))),
        Rep::None => Err(unsupported(node, "the truth of None")),
    }
}

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

/// The `int` of a literal, which has to fit in 31 bits.
fn int_literal(node: &impl Ranged, value: Option<i64>) -> Result<Expr, Error> {
    value
        .filter(|v| (INT_MIN..=INT_MAX).contains(v))
        .and_then(|v| i32::try_from(v).ok())
        .map(Expr::Int)
        .ok_or_else(|| unsupported(node, "an int that does not fit in 31 bits"))
}

fn unsupported(node: &impl Ranged, what: &str) -> Error {
    Error::Unsupported {
        range: node.range(),
        what: what.to_owned(),
    }
}
