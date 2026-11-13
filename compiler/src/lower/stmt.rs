//! Statements.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;
use ty_python_semantic::types::lowering::{Shape, expr_shape};

use super::call::is_builtins;
use super::class::generated_call;
use super::flow::imported_name;
use super::{Lowering, dict_part, params_of, unsupported};
use crate::Error;
use crate::cpython::Class;
use crate::ir::{ArithOp, CmpOp, Expr, FuncId, Place, Rep, Stmt};

/// The modules that a program may import. No import runs code in the
/// compiled program.
const MODULES: [&str; 4] = ["math", "enum", "dataclasses", "typing"];

/// The modules of [`MODULES`] that a program imports with `from` alone,
/// for a name that only an annotation reads.
const FROM_ONLY: [&str; 1] = ["typing"];

impl<'a> Lowering<'a, '_> {
    pub(super) fn body(&self, body: &'a [ast::Stmt]) -> Result<Vec<Stmt>, Error> {
        let mut lowered = Vec::new();
        for stmt in body {
            if let ast::Stmt::FunctionDef(def) = stmt
                && self.scope.borrow().is_none()
            {
                lowered.extend(self.defaults(def)?);
                let annotations = params_of(def)
                    .filter_map(|param| param.annotation())
                    .chain(def.returns.as_deref());
                self.check_annotations(annotations)?;
            }
            // A function does not evaluate the annotation of an assignment.
            if let ast::Stmt::AnnAssign(a) = stmt
                && self.scope.borrow().is_none()
            {
                self.check_annotations(std::iter::once(&*a.annotation))?;
            }
            if let ast::Stmt::ClassDef(def) = stmt {
                lowered.extend(self.class_body(def)?);
            }
            match stmt {
                ast::Stmt::For(f) => lowered.extend(self.for_(f)?),
                _ => lowered.extend(self.stmt(stmt)?),
            }
            if let ast::Stmt::FunctionDef(def) = stmt
                && self.scope.borrow().is_none()
            {
                lowered.extend(self.bind_current(def));
            }
        }
        Ok(lowered)
    }

    /// The assignment of the place of `def` among the `def`s of its name to
    /// the global that tells which function the name has, when the name has
    /// more than one `def`.
    fn bind_current(&self, def: &ast::StmtFunctionDef) -> Option<Stmt> {
        let func = self.def_func(def);
        let funcs = self.funcs.borrow();
        let (current, defs) = funcs.redefined.get(def.name.as_str())?;
        let place = defs.iter().position(|other| *other == func);
        let place = i32::try_from(place.expect("a def of the name")).expect("few defs");
        Some(Stmt::Assign {
            targets: vec![Place::Global(*current)],
            value: Expr::Int(place),
        })
    }

    /// The assignment of the default of each parameter of `def`, which the
    /// `def` evaluates once, in the order of the parameters.
    fn defaults(&self, def: &ast::StmtFunctionDef) -> Result<Vec<Stmt>, Error> {
        if !self.funcs.borrow().names.contains_key(def.name.as_str()) {
            return Ok(Vec::new());
        }
        self.default_values(def, self.def_func(def))
    }

    /// The assignment of the default of each parameter of `def`, the
    /// function `id`, in the order of the parameters.
    pub(super) fn default_values(
        &self,
        def: &ast::StmtFunctionDef,
        id: FuncId,
    ) -> Result<Vec<Stmt>, Error> {
        let sig = self.funcs.borrow().get(id).clone();
        params_of(def)
            .zip(&sig.defaults)
            .filter_map(|(param, var)| Some((param.default.as_deref()?, (*var)?)))
            .map(|(default, var)| {
                let rep = self.vars.borrow().get(var).rep;
                let value = self
                    .coerce(self.expr(default)?, rep)
                    .ok_or_else(|| unsupported(default, "a default of this type"))?;
                Ok(Stmt::Assign {
                    targets: vec![Place::Global(var)],
                    value,
                })
            })
            .collect()
    }

    fn stmt(&self, stmt: &'a ast::Stmt) -> Result<Option<Stmt>, Error> {
        match stmt {
            ast::Stmt::Expr(e) => {
                let value = self.expr(&e.value)?;
                // An example of a docstring writes the `repr` of the value
                // of each expression statement that is not `None`, as the
                // interactive prompt does.
                if self.in_doctest() && value.rep() != Rep::None {
                    // The prompt writes nothing for a value that is `None`
                    // when the example runs.
                    if self.may_be_none(value.rep(), &e.value) {
                        return Err(unsupported(&*e.value, "an example whose value may be None"));
                    }
                    let text = self.repr_of(value, e.range())?;
                    return Ok(Some(Stmt::Expr(Expr::Print {
                        args: vec![text],
                        site: self.site(e.range()),
                    })));
                }
                Ok(Some(Stmt::Expr(value)))
            }
            ast::Stmt::Assign(a) if a.targets.iter().any(|t| t.is_subscript_expr()) => {
                let [ast::Expr::Subscript(target)] = &*a.targets else {
                    return Err(unsupported(a, "this target"));
                };
                let container = self.expr(&target.value)?;
                if container.rep().class().is_some() {
                    let index = self.expr(&target.slice)?;
                    let value = self.expr(&a.value)?;
                    let call = self.special_setitem(target, value, container, index)?;
                    return Ok(Some(Stmt::Expr(call)));
                }
                match container.rep() {
                    // A builder of the prelude is a `str` that its code
                    // writes one code point at a time.
                    Rep::Str if self.in_prelude() => Ok(Some(Stmt::StrSet {
                        builder: container,
                        index: self.expr(&target.slice)?,
                        value: self.expr(&a.value)?,
                    })),
                    Rep::List(list) => {
                        let item = self.list_item(list);
                        let value = self
                            .coerce(self.expr(&a.value)?, item)
                            .ok_or_else(|| unsupported(&*a.value, "an item of this type"))?;
                        Ok(Some(Stmt::ListSet {
                            list: container,
                            index: self.int_operand(&target.slice)?,
                            value,
                            site: self.site(target.range()),
                        }))
                    }
                    _ => Err(unsupported(target, "an assignment to an item of this type")),
                }
            }
            ast::Stmt::Assign(a) if self.module_copy(&a.targets, &a.value).is_some() => {
                let (target, from) = self
                    .module_copy(&a.targets, &a.value)
                    .expect("a copy of a name of the module");
                let (from, rep) = self
                    .vars
                    .borrow()
                    .find(from)
                    .ok_or_else(|| unsupported(&*a.value, "a variable of this type"))?;
                // The copy asks whether `from` has a value.
                self.vars.borrow_mut().get_mut(from).flagged = true;
                let (to, _) = self.place(target, rep)?;
                Ok(Some(Stmt::Copy { from, to }))
            }
            ast::Stmt::Assign(a) => {
                let value = self.expr(&a.value)?;
                if let [ast::Expr::Attribute(attr)] = &*a.targets {
                    return self.assign_field(attr, value).map(Some);
                }
                let targets = a
                    .targets
                    .iter()
                    .map(|target| Ok(self.place(target, value.rep())?.0))
                    .collect::<Result<_, Error>>()?;
                Ok(Some(Stmt::Assign { targets, value }))
            }
            // An annotation alone binds nothing.
            ast::Stmt::AnnAssign(ast::StmtAnnAssign { value: None, .. }) => Ok(None),
            ast::Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                let (target, value) = self.target(target, self.expr(value)?)?;
                Ok(Some(Stmt::Assign {
                    targets: vec![target],
                    value,
                }))
            }
            ast::Stmt::AugAssign(ast::StmtAugAssign {
                target, op, value, ..
            }) if let ast::Expr::Attribute(attr) = &**target => {
                let (object, class, field, rep) = self.updated_field(attr)?;
                let left = self.object_attribute(attr, Expr::Target(class), class)?;
                let left = self.narrowed_field(target, attr, left)?;
                no_list_update(&left, stmt)?;
                let right = self.expr(value)?;
                let value = self.augmented(stmt, *op, (left, &**target), (right, &**value))?;
                let value = self
                    .coerce(value, rep)
                    .ok_or_else(|| unsupported(attr, "a field with values of two types"))?;
                Ok(Some(Stmt::UpdateField {
                    object,
                    class,
                    field,
                    value,
                }))
            }
            ast::Stmt::AugAssign(a) => {
                let ast::Expr::Name(name) = &*a.target else {
                    return Err(unsupported(&*a.target, "this target"));
                };
                let left = self.load(name)?;
                no_list_update(&left, a)?;
                let right = self.expr(&a.value)?;
                let value = self.augmented(a, a.op, (left, name), (right, &*a.value))?;
                let (target, value) = self.target(&a.target, value)?;
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
                    return self.inner_def(def).map(Some);
                }
                Ok(Some(Stmt::Def(self.def_func(def))))
            }
            ast::Stmt::ClassDef(def) => Ok(Some(Stmt::Class(self.class_of(def)?))),
            ast::Stmt::Return(r) => self.return_(r).map(Some),
            ast::Stmt::Raise(r) => self.raise(r).map(Some),
            ast::Stmt::Assert(a) => {
                let arg = a
                    .msg
                    .as_deref()
                    .map(|msg| self.exception_arg(msg))
                    .transpose()?;
                // The traceback marks the condition, and the program
                // evaluates the text only when the condition is false.
                Ok(Some(Stmt::If {
                    cond: self.truth_of(&a.test)?,
                    then: Vec::new(),
                    else_: vec![Stmt::Raise {
                        class: Class::AssertionError,
                        arg,
                        site: self.site(a.test.range()),
                    }],
                }))
            }
            ast::Stmt::Import(i) => {
                if let Some(alias) = i.names.iter().find(|alias| {
                    let name = alias.name.as_str();
                    !MODULES.contains(&name) || FROM_ONLY.contains(&name)
                }) {
                    return Err(unsupported(alias, "an import of this module"));
                }
                self.import(stmt, &i.names, false)
            }
            ast::Stmt::ImportFrom(i) => {
                let module = i.module.as_ref().map(|module| module.as_str());
                let Some(module) = module.filter(|module| i.level == 0 && MODULES.contains(module))
                else {
                    return Err(unsupported(stmt, "an import from this module"));
                };
                if let Some(alias) = i.names.iter().find(|alias| alias.name.as_str() == "*") {
                    return Err(unsupported(alias, "an import of every name"));
                }
                // The compiler reads `Enum`, `auto` and `dataclass` in the
                // form of a class alone, `Callable` in an annotation alone,
                // and every name of `math` at its use.
                let known = |name: &str| match module {
                    "enum" => ["Enum", "auto"].contains(&name),
                    "dataclasses" => name == "dataclass",
                    "typing" => name == "Callable",
                    _ => true,
                };
                if let Some(alias) = i.names.iter().find(|alias| !known(alias.name.as_str())) {
                    return Err(unsupported(alias, "an import of this name"));
                }
                self.import(stmt, &i.names, true)
            }
            _ => Err(unsupported(stmt, "this statement")),
        }
    }

    /// Returns `true` if `node`, a value of `rep`, may be `None`, `false`
    /// otherwise. A uniform value may be `None` unless its type is a union
    /// without `None`.
    fn may_be_none(&self, rep: Rep, node: &ast::Expr) -> bool {
        match rep {
            Rep::StrOrNone | Rep::ObjectOrNone(_) | Rep::ListOrNone(_) => true,
            Rep::Uniform => match expr_shape(self.model(), node) {
                Some(Shape::Union(shapes)) => shapes.iter().any(|s| matches!(s, Shape::None)),
                _ => true,
            },
            _ => false,
        }
    }

    fn for_(&self, f: &'a ast::StmtFor) -> Result<Vec<Stmt>, Error> {
        if f.is_async {
            return Err(unsupported(f, "this statement"));
        }
        self.for_loop(&f.target, &f.iter, &mut || self.body(&f.body), &f.orelse)
    }

    /// `for target in iter` with the code that `body` gives, which the
    /// lowering lowers once the target has its value, and the `else` of
    /// `else_`. A loop over a list or a `str` is a `while` over an index.
    /// The test reads the length of the list each time, so an `append` in
    /// the body makes the loop longer, as the iterator of CPython does.
    /// The iterable runs once, before the loop.
    pub(super) fn for_loop(
        &self,
        target: &'a ast::Expr,
        iter: &'a ast::Expr,
        body: &mut dyn FnMut() -> Result<Vec<Stmt>, Error>,
        else_: &'a [ast::Stmt],
    ) -> Result<Vec<Stmt>, Error> {
        if let Some(range) = self.range_call(iter) {
            return self
                .for_range(target, range, body, else_)
                .map(|stmt| vec![stmt]);
        }
        if let Some(stmts) = self.for_dict(target, iter, body, else_)? {
            return Ok(stmts);
        }
        let seq = self.expr(iter)?;
        let rep = seq.rep();
        if !matches!(rep, Rep::List(_) | Rep::Str) {
            return Err(unsupported(iter, "a for over this type"));
        }
        let site = self.site(iter.range());
        let (seq_var, index) = (self.hidden_var(rep), self.hidden_var(Rep::Int));
        let read = |place, rep| {
            Box::new(Expr::Load {
                place,
                rep,
                check: None,
            })
        };
        let item = match rep {
            Rep::List(list) => Expr::Item {
                list: read(seq_var, rep),
                index: read(index, Rep::Int),
                rep: self.list_item(list),
            },
            _ => Expr::Index {
                value: read(seq_var, rep),
                index: read(index, Rep::Int),
                site,
            },
        };
        let (place, item) = self.target(target, item)?;
        let mut lowered = vec![
            Stmt::Assign {
                targets: vec![place],
                value: item,
            },
            Stmt::Assign {
                targets: vec![index],
                value: Expr::Arith {
                    op: ArithOp::Add,
                    left: read(index, Rep::Int),
                    right: Box::new(Expr::Int(1)),
                    site,
                },
            },
        ];
        lowered.extend(body()?);
        let len = Expr::Len {
            operand: read(seq_var, rep),
            site,
        };
        Ok(vec![
            Stmt::Assign {
                targets: vec![seq_var],
                value: seq,
            },
            Stmt::Assign {
                targets: vec![index],
                value: Expr::Int(0),
            },
            Stmt::While {
                cond: Expr::Compare(read(index, Rep::Int), vec![(CmpOp::Lt, len)]),
                body: lowered,
                else_: self.body(else_)?,
            },
        ])
    }

    /// `for` over a `dict`, over its `keys()`, over its `values()`, or
    /// over its `items()` with two names as the target, or over a `set`,
    /// and `None` for a `for` over another value. An object of `DictIter`
    /// or of `SetIter` of the prelude holds the place of the loop, and its
    /// method `next` raises the errors of a `dict` or a `set` that changes,
    /// at the iterable.
    fn for_dict(
        &self,
        target: &'a ast::Expr,
        iter: &'a ast::Expr,
        body: &mut dyn FnMut() -> Result<Vec<Stmt>, Error>,
        else_: &'a [ast::Stmt],
    ) -> Result<Option<Vec<Stmt>>, Error> {
        let (value, part) = dict_part(iter);
        match expr_shape(self.model(), value) {
            Some(Shape::Dict(..)) => {}
            Some(Shape::Set(_)) if part == "keys" => {}
            _ => return Ok(None),
        }
        let dict = self.expr(value)?;
        let Rep::Object(class) = dict.rep() else {
            return Err(unsupported(iter, "a for over this type"));
        };
        let site = self.site(iter.range());
        let iter = self.method_func(class, "iter").expect("a copy with iter");
        let iter_rep = self.funcs.borrow().get(iter).ret;
        let iter_class = iter_rep.class().expect("an object of an iterator");
        let iter_var = self.hidden_var(iter_rep);
        let read = || Expr::Load {
            place: iter_var,
            rep: iter_rep,
            check: None,
        };
        let call = |name: &str| {
            let func = self
                .method_func(iter_class, name)
                .expect("a method of the iterator");
            let rep = self.funcs.borrow().get(func).ret;
            generated_call(func, vec![read()], rep, site)
        };
        let mut lowered = match (part, target) {
            ("items", ast::Expr::Tuple(tuple)) if tuple.elts.len() == 2 => {
                let mut assigns = Vec::new();
                for (target, name) in tuple.elts.iter().zip(["key", "value"]) {
                    let (place, value) = self.target(target, call(name))?;
                    assigns.push(Stmt::Assign {
                        targets: vec![place],
                        value,
                    });
                }
                assigns
            }
            ("items", target) => return Err(unsupported(target, "this target")),
            (part, target) => {
                let name = if part == "keys" { "key" } else { "value" };
                let (place, value) = self.target(target, call(name))?;
                vec![Stmt::Assign {
                    targets: vec![place],
                    value,
                }]
            }
        };
        lowered.extend(body()?);
        Ok(Some(vec![
            Stmt::Assign {
                targets: vec![iter_var],
                value: generated_call(iter, vec![dict], iter_rep, site),
            },
            Stmt::While {
                cond: call("next"),
                body: lowered,
                else_: self.body(else_)?,
            },
        ]))
    }

    /// The call of `range` that `iter` is, which only a `for` takes.
    pub(super) fn range_call<'e>(&self, iter: &'e ast::Expr) -> Option<&'e ast::ExprCall> {
        let ast::Expr::Call(call) = iter else {
            return None;
        };
        (self.builtin_name(&call.func).as_deref() == Some("range")).then_some(call)
    }

    /// `for target in range(...)`, with the arguments in the order of the
    /// call. `range(stop)` starts at 0, and the step is 1 without a third
    /// argument.
    fn for_range(
        &self,
        target: &'a ast::Expr,
        range: &ast::ExprCall,
        body: &mut dyn FnMut() -> Result<Vec<Stmt>, Error>,
        else_: &'a [ast::Stmt],
    ) -> Result<Stmt, Error> {
        if !range.arguments.keywords.is_empty() {
            return Err(unsupported(range, "a range with these arguments"));
        }
        let args = range
            .arguments
            .args
            .iter()
            .map(|arg| self.int_operand(arg))
            .collect::<Result<Vec<_>, Error>>()?;
        let mut args = args.into_iter();
        let (start, stop, step) = match (args.next(), args.next(), args.next(), args.next()) {
            (Some(stop), None, None, None) => (Expr::Int(0), stop, Expr::Int(1)),
            (Some(start), Some(stop), None, None) => (start, stop, Expr::Int(1)),
            (Some(start), Some(stop), Some(step), None) => (start, stop, step),
            _ => return Err(unsupported(range, "a range with these arguments")),
        };
        let (var, _) = self.place(target, Rep::Int)?;
        Ok(Stmt::ForRange {
            var,
            start,
            stop,
            step,
            body: body()?,
            else_: self.body(else_)?,
            site: self.site(range.range()),
        })
    }

    /// The target and the name of the global of `a`, when `a` is a copy
    /// of a name of the module that the examples of the docstrings assign.
    /// The runner gives the examples a copy of the globals of the module,
    /// and a name without a value stays without one in the copy.
    pub(super) fn module_copy<'s>(
        &self,
        targets: &'s [ast::Expr],
        value: &'s ast::Expr,
    ) -> Option<(&'s ast::Expr, &'s str)> {
        let ([target @ ast::Expr::Name(to)], ast::Expr::Name(from)) = (targets, value) else {
            return None;
        };
        let copies = self.tests.as_ref()?.copies;
        let copy = |name: &ast::ExprName| copies.iter().any(|copy| *copy == name.id.as_str());
        (copy(to) || copy(from)).then_some((target, from.id.as_str()))
    }

    /// The text of an exception that `arg` gives, which has to be a `str`.
    fn exception_arg(&self, arg: &ast::Expr) -> Result<Expr, Error> {
        let lowered = self.expr(arg)?;
        if lowered.rep() != Rep::Str {
            return Err(unsupported(arg, "an exception with this argument"));
        }
        Ok(lowered)
    }

    /// The import of the names of `aliases`, which the module declares.
    fn import(
        &self,
        stmt: &ast::Stmt,
        aliases: &[ast::Alias],
        from: bool,
    ) -> Result<Option<Stmt>, Error> {
        if self.scope.borrow().is_some() {
            return Err(unsupported(stmt, "an import in a function"));
        }
        let imports = self.imports.borrow();
        let ids = aliases
            .iter()
            .map(|alias| {
                *imports
                    .ids
                    .get(imported_name(alias, from))
                    .expect("a declared import")
            })
            .collect();
        Ok(Some(Stmt::Import(ids)))
    }

    /// `raise C`, `raise C()` or `raise C(text)` of a class of the builtins,
    /// with a `str` as the text. A `KeyError` takes a value of any type,
    /// and its text is the `repr` of the value.
    fn raise(&self, r: &ast::StmtRaise) -> Result<Stmt, Error> {
        let (Some(exc), None) = (r.exc.as_deref(), &r.cause) else {
            return Err(unsupported(r, "this raise"));
        };
        let (class, args) = match exc {
            ast::Expr::Call(call) if call.arguments.keywords.is_empty() => {
                (&*call.func, &*call.arguments.args)
            }
            ast::Expr::Call(_) => return Err(unsupported(exc, "a raise with these arguments")),
            class => (class, &[][..]),
        };
        let class = match expr_shape(self.model(), class) {
            Some(Shape::Class(id)) if is_builtins(self.model().db(), id.file) => {
                Class::of(&id.name)
            }
            _ => None,
        };
        let Some(class) = class else {
            return Err(unsupported(exc, "a raise of this exception"));
        };
        let arg = match args {
            [] => None,
            [arg] if class == Class::KeyError => {
                let value = self.expr(arg)?;
                Some(self.repr_of(value, arg.range())?)
            }
            [arg] => Some(self.exception_arg(arg)?),
            _ => return Err(unsupported(exc, "a raise with these arguments")),
        };
        Ok(Stmt::Raise {
            class,
            arg,
            site: self.site(r.range()),
        })
    }

    fn return_(&self, r: &ast::StmtReturn) -> Result<Stmt, Error> {
        let ret = match self.scope.borrow().as_ref() {
            Some(scope) if scope.doctest.is_none() => scope.ret,
            _ => return Err(unsupported(r, "a return outside a function")),
        };
        match r.value.as_deref() {
            None | Some(ast::Expr::NoneLiteral(_)) if ret == Rep::None => Ok(Stmt::Return(None)),
            None => self
                .coerce(Expr::None, ret)
                .map(|value| Stmt::Return(Some(value)))
                .ok_or_else(|| unsupported(r, "a return of None")),
            Some(value) => self
                .coerce(self.expr(value)?, ret)
                .map(|value| Stmt::Return(Some(value)))
                .ok_or_else(|| unsupported(value, "a return of this type")),
        }
    }
}

/// Refuses an operator with assignment, as `xs += ys`, whose target `left`
/// is a list. CPython changes the list in place, where `+` gives a new one.
fn no_list_update(left: &Expr, node: &impl Ranged) -> Result<(), Error> {
    match left.rep().list() {
        Some(_) => Err(unsupported(node, "an operator with assignment on a list")),
        None => Ok(()),
    }
}
