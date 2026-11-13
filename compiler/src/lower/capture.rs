//! What the functions inside a function capture from it: the variables of
//! the function that each one reads, and which of them live in a cell.

use std::collections::{HashMap, HashSet};

use ruff_python_ast as ast;
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_text_size::{Ranged, TextRange};

use super::flow::{Bindings, names_in};

/// The captures of the functions inside a body.
#[derive(Default)]
pub(super) struct Captures {
    /// The variables of the body that live in a cell.
    pub cells: HashSet<String>,
    /// The variables of the body that each function inside it captures, by
    /// the range of the function, in the order of their first read. Each
    /// comes with `true` when the function may read it before it has a
    /// value.
    pub funcs: HashMap<TextRange, Vec<(String, bool)>>,
}

/// The captures of the functions inside `body`, whose parameters are
/// `params` and whose locals are the names that `is_local` accepts.
///
/// A captured variable holds a copy of its value when it has one binding,
/// outside a loop, and has a value at each capture, since then the value
/// never changes after a capture. Any other captured variable lives in a
/// cell, which the function and the code of the value share.
pub(super) fn captures<'a>(
    params: &[&'a str],
    body: &'a [ast::Stmt],
    is_local: &dyn Fn(&str) -> bool,
) -> Captures {
    let mut walk = Walk {
        is_local,
        bindings: params.iter().map(|param| (*param, 1)).collect(),
        loops: 0,
        found: Vec::new(),
    };
    let mut assigned: HashSet<&str> = params.iter().copied().collect();
    walk.stmts(body, &mut assigned);
    // Whether each captured variable has a value at every capture.
    let mut always: HashMap<&str, bool> = HashMap::new();
    for capture in walk.found.iter().flat_map(|(_, names)| names) {
        *always.entry(capture.name).or_insert(true) &= capture.before;
    }
    let cells = always
        .into_iter()
        .filter(|(name, always)| !(*always && walk.bindings.get(name).copied() == Some(1)))
        .map(|(name, _)| name.to_owned())
        .collect();
    let funcs = walk
        .found
        .into_iter()
        .map(|(range, names)| {
            let names = names
                .into_iter()
                .map(|capture| {
                    // The `def` of a function that reads its own name binds
                    // the name before the function can run.
                    let unbound = !capture.before && !capture.own;
                    (capture.name.to_owned(), unbound)
                })
                .collect();
            (range, names)
        })
        .collect();
    Captures { cells, funcs }
}

/// A variable that a function inside the body captures.
struct Found<'a> {
    name: &'a str,
    /// Whether the variable has a value on every path to the capture.
    before: bool,
    /// Whether the function is the `def` that binds the variable.
    own: bool,
}

struct Walk<'a, 'f> {
    is_local: &'f dyn Fn(&str) -> bool,
    /// The number of bindings of each local, where a binding in a loop
    /// counts as two, since it may run more than once.
    bindings: HashMap<&'a str, u32>,
    /// The number of loops around the code.
    loops: u32,
    /// The captures of each function inside the body, by its range.
    found: Vec<(TextRange, Vec<Found<'a>>)>,
}

impl<'a> Walk<'a, '_> {
    /// Walks `body`, with `assigned` the names that have a value on every
    /// path to it, and adds to it the names that have one after it.
    fn stmts(&mut self, body: &'a [ast::Stmt], assigned: &mut HashSet<&'a str>) {
        for stmt in body {
            self.stmt(stmt, assigned);
        }
    }

    fn stmt(&mut self, stmt: &'a ast::Stmt, assigned: &mut HashSet<&'a str>) {
        match stmt {
            ast::Stmt::Assign(a) => {
                self.exprs(&a.value, assigned);
                for target in &a.targets {
                    self.target(target, assigned);
                }
            }
            ast::Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    self.exprs(value, assigned);
                    self.target(&a.target, assigned);
                }
            }
            ast::Stmt::AugAssign(a) => {
                self.exprs(&a.value, assigned);
                self.target(&a.target, assigned);
            }
            ast::Stmt::If(i) => {
                self.exprs(&i.test, assigned);
                let mut outs = Vec::new();
                let mut branch = assigned.clone();
                self.stmts(&i.body, &mut branch);
                outs.push(branch);
                for clause in &i.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        self.exprs(test, assigned);
                    }
                    let mut branch = assigned.clone();
                    self.stmts(&clause.body, &mut branch);
                    outs.push(branch);
                }
                // Without an `else`, a path goes through no branch.
                let complete = i
                    .elif_else_clauses
                    .last()
                    .is_some_and(|clause| clause.test.is_none());
                if complete {
                    let mut outs = outs.into_iter();
                    let first = outs.next().expect("an if has a body");
                    *assigned = outs.fold(first, |all, out| &all & &out);
                }
            }
            // The body of a loop may never run, and a later run of the body
            // sees what an earlier one assigned.
            ast::Stmt::While(w) => {
                self.exprs(&w.test, assigned);
                self.loops += 1;
                self.stmts(&w.body, &mut assigned.clone());
                self.loops -= 1;
                self.stmts(&w.orelse, &mut assigned.clone());
            }
            ast::Stmt::For(f) => {
                self.exprs(&f.iter, assigned);
                self.loops += 1;
                let mut body = assigned.clone();
                self.target(&f.target, &mut body);
                self.stmts(&f.body, &mut body);
                self.loops -= 1;
                self.stmts(&f.orelse, &mut assigned.clone());
            }
            ast::Stmt::FunctionDef(def) => {
                for param in def.parameters.iter_non_variadic_params() {
                    if let Some(default) = &param.default {
                        self.exprs(default, assigned);
                    }
                }
                let names = free_names_of_def(def);
                self.capture(def.range(), names, Some(def.name.as_str()), assigned);
                self.bind(def.name.as_str());
                assigned.insert(def.name.as_str());
            }
            ast::Stmt::Expr(e) => self.exprs(&e.value, assigned),
            ast::Stmt::Return(r) => {
                if let Some(value) = &r.value {
                    self.exprs(value, assigned);
                }
            }
            ast::Stmt::Raise(r) => {
                if let Some(exc) = &r.exc {
                    self.exprs(exc, assigned);
                }
            }
            ast::Stmt::Assert(a) => {
                self.exprs(&a.test, assigned);
                if let Some(msg) = &a.msg {
                    self.exprs(msg, assigned);
                }
            }
            _ => {}
        }
    }

    /// Binds each name of `target`, after the expressions in it run.
    fn target(&mut self, target: &'a ast::Expr, assigned: &mut HashSet<&'a str>) {
        match target {
            ast::Expr::Name(name) => {
                self.bind(name.id.as_str());
                assigned.insert(name.id.as_str());
            }
            ast::Expr::Tuple(tuple) => {
                for elt in &tuple.elts {
                    self.target(elt, assigned);
                }
            }
            target => self.exprs(target, assigned),
        }
    }

    fn bind(&mut self, name: &'a str) {
        let count = if self.loops > 0 { 2 } else { 1 };
        *self.bindings.entry(name).or_insert(0) += count;
    }

    /// Finds each `lambda` and each generator expression of `expr`, which
    /// capture what they read.
    fn exprs(&mut self, expr: &'a ast::Expr, assigned: &HashSet<&'a str>) {
        #[derive(Default)]
        struct Functions<'a> {
            lambdas: Vec<&'a ast::ExprLambda>,
            generators: Vec<&'a ast::ExprGenerator>,
        }
        impl<'a> Visitor<'a> for Functions<'a> {
            fn visit_expr(&mut self, expr: &'a ast::Expr) {
                match expr {
                    ast::Expr::Lambda(lambda) => self.lambdas.push(lambda),
                    // The first iterable runs in the code around the
                    // generator.
                    ast::Expr::Generator(generator) => {
                        self.generators.push(generator);
                        if let Some(first) = generator.generators.first() {
                            self.visit_expr(&first.iter);
                        }
                    }
                    expr => walk_expr(self, expr),
                }
            }
        }
        let mut functions = Functions::default();
        functions.visit_expr(expr);
        for lambda in functions.lambdas {
            self.capture(lambda.range(), free_names_of_lambda(lambda), None, assigned);
        }
        for generator in functions.generators {
            let names = free_names_of_generator(generator);
            self.capture(generator.range(), names, None, assigned);
        }
    }

    /// Records what the function at `range` captures of the names `names`
    /// that it reads. `own` is the name that the function binds, for a
    /// `def`.
    fn capture(
        &mut self,
        range: TextRange,
        names: Vec<&'a str>,
        own: Option<&str>,
        assigned: &HashSet<&'a str>,
    ) {
        let found = names
            .into_iter()
            .filter(|name| (self.is_local)(name))
            .map(|name| Found {
                name,
                before: assigned.contains(name),
                own: own == Some(name),
            })
            .collect();
        self.found.push((range, found));
    }
}

/// Each name that `lambda` reads and does not bind, once, in the order of
/// the code, with the names that a function inside it reads and does not
/// bind.
pub(super) fn free_names_of_lambda(lambda: &ast::ExprLambda) -> Vec<&str> {
    let mut free = Free::default();
    free.lambda(lambda);
    free.names()
}

/// [`free_names_of_lambda`] of the function of the generator expression
/// `generator`, which takes the first iterable from the code around it.
pub(super) fn free_names_of_generator(generator: &ast::ExprGenerator) -> Vec<&str> {
    let mut free = Free::default();
    free.comprehension(&generator.generators, &[&generator.elt], true);
    free.names()
}

/// [`free_names_of_lambda`] of the function of `def`. A name that the
/// function declares `global` is not a variable of the function around it.
pub(super) fn free_names_of_def(def: &ast::StmtFunctionDef) -> Vec<&str> {
    let mut free = Free::default();
    free.def(def);
    free.names()
}

#[derive(Default)]
struct Free<'a> {
    /// The names that each function around the code binds.
    bound: Vec<HashSet<&'a str>>,
    found: Vec<&'a str>,
}

impl<'a> Free<'a> {
    fn names(self) -> Vec<&'a str> {
        let mut seen = HashSet::new();
        self.found
            .into_iter()
            .filter(|name| seen.insert(*name))
            .collect()
    }

    fn lambda(&mut self, lambda: &'a ast::ExprLambda) {
        let params = lambda
            .parameters
            .iter()
            .flat_map(|parameters| parameters.iter_non_variadic_params())
            .map(|param| param.parameter.name.as_str())
            .collect();
        self.bound.push(params);
        self.visit_expr(&lambda.body);
        self.bound.pop();
    }

    fn def(&mut self, def: &'a ast::StmtFunctionDef) {
        let bindings = Bindings::of(&def.body);
        let bound = def
            .parameters
            .iter_non_variadic_params()
            .map(|param| param.parameter.name.as_str())
            .chain(bindings.assigned.iter().copied())
            .chain(bindings.globals.iter().copied())
            .collect();
        self.bound.push(bound);
        for stmt in &def.body {
            self.visit_stmt(stmt);
        }
        self.bound.pop();
    }
}

impl<'a> Visitor<'a> for Free<'a> {
    fn visit_stmt(&mut self, stmt: &'a ast::Stmt) {
        match stmt {
            // The defaults run in the function around the `def`.
            ast::Stmt::FunctionDef(def) => {
                for param in def.parameters.iter_non_variadic_params() {
                    if let Some(default) = &param.default {
                        self.visit_expr(default);
                    }
                }
                self.def(def);
            }
            stmt => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        match expr {
            ast::Expr::Name(name)
                if name.ctx.is_load()
                    && !self
                        .bound
                        .iter()
                        .any(|bound| bound.contains(name.id.as_str())) =>
            {
                self.found.push(name.id.as_str());
            }
            ast::Expr::Lambda(lambda) => self.lambda(lambda),
            ast::Expr::ListComp(comp) => {
                self.comprehension(&comp.generators, &[&comp.elt], false);
            }
            ast::Expr::SetComp(comp) => {
                self.comprehension(&comp.generators, &[&comp.elt], false);
            }
            ast::Expr::Generator(comp) => {
                self.comprehension(&comp.generators, &[&comp.elt], false);
            }
            ast::Expr::DictComp(comp) => {
                let values: Vec<&ast::Expr> = comp
                    .key
                    .as_deref()
                    .into_iter()
                    .chain([&*comp.value])
                    .collect();
                self.comprehension(&comp.generators, &values, false);
            }
            expr => walk_expr(self, expr),
        }
    }
}

impl<'a> Free<'a> {
    /// Visits a comprehension, whose first iterable reads the names of the
    /// code around it, and skips the first iterable when `inside`. The
    /// names that a loop binds are not free in the tests, the iterables and
    /// the values after it.
    fn comprehension(
        &mut self,
        generators: &'a [ast::Comprehension],
        values: &[&'a ast::Expr],
        inside: bool,
    ) {
        let mut bound = 0;
        for (i, generator) in generators.iter().enumerate() {
            if !(inside && i == 0) {
                self.visit_expr(&generator.iter);
            }
            let mut names = HashSet::new();
            for name in names_in(&generator.target) {
                names.insert(name.id.as_str());
            }
            self.bound.push(names);
            bound += 1;
            for test in &generator.ifs {
                self.visit_expr(test);
            }
        }
        for value in values {
            self.visit_expr(value);
        }
        self.bound.truncate(self.bound.len() - bound);
    }
}
