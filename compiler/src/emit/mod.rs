//! From the IR to the bytes of the module.

mod layout;
pub(crate) mod module;
mod repr;
mod runtime;

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, ValType};

use crate::cpython::{Class, Message, RANGE_STEP_ZERO, RECURSION_ERROR};
use crate::ir::{
    Binding, Capture, Captured, Check, ClassId, CmpOp, Expr, Func, FuncId, FuncTypeId, ImportId,
    LogicOp, MinMaxOp, NameError, Place, Program, Rep, Site, Stmt, Tolerance, Undefined, VarId,
};
use module::{CodeMap, FuncIdx, FuncState, GlobalIdx, Label, LocalIdx, ModuleState, TypeIdx};
use repr::{Reprs, Types};
use runtime::host::Host;

/// The bytes of the module, with the place of the code of each site.
pub(crate) fn emit(program: &Program) -> (Vec<u8>, CodeMap) {
    let mut m = ModuleState::new();
    let host = Host::new(&mut m, program.tests.is_some());
    // The struct of the values of each function that captures variables,
    // and the rep of each cell.
    let closures: Vec<(FuncTypeId, &[Capture])> = program
        .funcs
        .iter()
        .filter(|func| !func.captures.is_empty())
        .map(|func| {
            let ty = func.value.expect("the code of a value captures");
            (ty, func.captures.as_slice())
        })
        .collect();
    let mut closure = 0;
    let closure_of = program
        .funcs
        .iter()
        .map(|func| {
            (!func.captures.is_empty()).then(|| {
                closure += 1;
                closure - 1
            })
        })
        .collect();
    let mut cells: Vec<Rep> = Vec::new();
    let reps = program.funcs.iter().flat_map(|func| {
        let vars = func.vars.iter().filter(|var| var.cell).map(|var| var.rep);
        let captures = func
            .captures
            .iter()
            .filter(|capture| capture.cell)
            .map(|capture| capture.rep);
        vars.chain(captures)
    });
    for rep in reps {
        if !cells.contains(&rep) {
            cells.push(rep);
        }
    }
    let mut reprs = Reprs::new(
        &mut m,
        &host,
        program.tests.is_some(),
        &Types {
            classes: &program.classes,
            lists: &program.lists,
            funcs: &program.func_types,
            closures: &closures,
            cells: &cells,
        },
    );
    let vars = program
        .vars
        .iter()
        .map(|var| Slots {
            cell: None,
            value: reprs.global(&mut m, var.rep),
            defined: var
                .flagged
                .then(|| m.global(ValType::I32, &ConstExpr::i32_const(0))),
        })
        .collect();
    let mut funcs: Vec<Option<FuncSlots>> = program
        .funcs
        .iter()
        .map(|func| {
            if func.alias.is_some() {
                return None;
            }
            let params: Vec<ValType> = func
                .vars
                .iter()
                .take(func.params as usize)
                .map(|var| reprs.local_type(var.rep).expect("a parameter has a value"))
                .collect();
            let results: Vec<ValType> = reprs.value_type(func.ret).into_iter().collect();
            let ty = match func.value {
                Some(ty) => reprs.func_code(ty),
                None => m.func_type(&params, &results),
            };
            Some(FuncSlots {
                index: m.reserve(ty),
                // No `def` binds a function of the prelude.
                defined: func
                    .frame
                    .then(|| m.global(ValType::I32, &ConstExpr::i32_const(0))),
            })
        })
        .collect();
    for (i, func) in program.funcs.iter().enumerate() {
        if let Some(alias) = func.alias {
            let slots = *funcs
                .get(alias.index())
                .and_then(Option::as_ref)
                .expect("a function with code of its own");
            *funcs.get_mut(i).expect("a function of the program") = Some(slots);
        }
    }
    let funcs: Vec<FuncSlots> = funcs
        .into_iter()
        .map(|slots| slots.expect("the slots of each function"))
        .collect();
    let index = |func: Option<FuncId>| {
        func.map(|func| {
            funcs
                .get(func.index())
                .expect("a function of the program")
                .index
        })
    };
    reprs.set_class_strs(
        program
            .classes
            .iter()
            .map(|class| index(class.str))
            .collect(),
    );
    reprs.set_reprs(
        program
            .classes
            .iter()
            .map(|class| index(class.repr))
            .collect(),
        index(program.str_repr),
    );
    reprs.set_list_reprs(program.lists.iter().map(|list| index(list.repr)).collect());
    reprs.set_copy_eqs(
        program
            .classes
            .iter()
            .enumerate()
            .filter_map(|(i, class)| Some((ClassId::new(i), index(class.copy_eq)?)))
            .collect(),
    );
    // The flag of each name that an import binds, as the flag of a `def`.
    let imports = program
        .imports
        .iter()
        .map(|_| m.global(ValType::I32, &ConstExpr::i32_const(0)))
        .collect();
    // The flag of each class, as the flag of a `def`.
    let classes = program
        .classes
        .iter()
        .map(|_| m.global(ValType::I32, &ConstExpr::i32_const(0)))
        .collect();
    let ordered = program.vars.iter().any(|var| var.ordered)
        || program.funcs.iter().any(|func| func.ordered)
        || program.imports.iter().any(|import| import.ordered)
        || program.classes.iter().any(|class| class.ordered);
    let order = ordered.then(|| m.global(ValType::I32, &ConstExpr::i32_const(0)));
    let mut f = FuncState::new(0);
    let site = f.local(ValType::I32);
    let mut e = Emitter {
        m,
        host,
        reprs,
        program,
        vars,
        funcs,
        imports,
        classes,
        order,
        values: HashMap::new(),
        closure_of,
        code: Code {
            func: None,
            site: Some(site),
            loops: Vec::new(),
            vars: Vec::new(),
            target: None,
            chain: None,
            logic: None,
            env: None,
        },
    };
    let start_ty = e.m.func_type(&[], &[]);
    let start = e.m.reserve(start_ty);
    f.ins().call(e.host.start);
    e.reprs.begin(&mut f);
    for stmt in &program.body {
        e.stmt(&mut f, stmt);
    }
    if let Some(tests) = &program.tests {
        e.reprs.tests_run(&mut f);
        for stmt in tests {
            e.stmt(&mut f, stmt);
        }
    }
    e.reprs.end_module(&mut f, site);
    for (i, func) in program.funcs.iter().enumerate() {
        if func.alias.is_none() {
            e.func(FuncId::new(i), func);
        }
    }
    e.reprs.finish(&mut e.m, &program.sites);
    let init = e.m.take_init();
    e.m.define_frame(start, f, &init);
    e.m.export_func("_start", start);
    e.m.finish(start)
}

struct Emitter<'a> {
    m: ModuleState,
    host: Host,
    reprs: Reprs,
    program: &'a Program,
    /// The globals of each variable of the module.
    vars: Vec<Slots<GlobalIdx>>,
    funcs: Vec<FuncSlots>,
    /// The global that is not 0 once the import has run, for each name
    /// that an import binds.
    imports: Vec<GlobalIdx>,
    /// The global that is not 0 once the `class` statement has run, for
    /// each class.
    classes: Vec<GlobalIdx>,
    /// The number of ordered globals with a value, which a global gets as
    /// its flag at its first binding, when the program has an ordered
    /// global.
    order: Option<GlobalIdx>,
    /// The global of the value of each function with
    /// [`Func::value`] that the program reads.
    values: HashMap<FuncId, GlobalIdx>,
    /// The place of each function that captures variables among them, by
    /// the id of the function.
    closure_of: Vec<Option<usize>>,
    code: Code,
}

/// The state of the code that the emitter is in, the module or a function.
struct Code {
    /// The function, and `None` in the module.
    func: Option<FuncId>,
    /// The local that holds the site of the instruction that may raise,
    /// and `None` in a function without a frame.
    site: Option<LocalIdx>,
    /// The target of `break` and of `continue`, for each loop
    /// that encloses the current statement.
    loops: Vec<Loop>,
    /// The locals of each variable of the function.
    vars: Vec<Slots<LocalIdx>>,
    /// The local that holds the object of the [`Stmt::UpdateField`] that
    /// the emitter is in.
    target: Option<LocalIdx>,
    /// The locals of the left and the right value of the test of the
    /// [`Expr::Chain`] that the emitter is in.
    chain: Option<(LocalIdx, LocalIdx)>,
    /// The local of the left value of the [`Expr::Logic`] that the emitter
    /// is in, and `None` for a left value of `None`.
    logic: Option<LocalIdx>,
    /// The local that holds the value of the function, as its own struct,
    /// for the code of a value that captures variables.
    env: Option<(LocalIdx, TypeIdx)>,
}

struct Loop {
    break_: Label,
    continue_: Label,
}

/// The globals of a variable of the module, or the locals of a variable of
/// a function.
struct Slots<I> {
    value: I,
    /// The local of the cell that holds the value, for a variable in a
    /// cell.
    cell: Option<I>,
    /// Not 0 once the variable has a value, for a flagged variable. An
    /// ordered variable holds the place of its first binding among the
    /// ordered globals, from 1.
    defined: Option<I>,
}

#[derive(Clone, Copy)]
struct FuncSlots {
    index: FuncIdx,
    /// The global that is not 0 once the `def` has run, as the flag of a
    /// variable. A function without a frame has none.
    defined: Option<GlobalIdx>,
}

impl Emitter<'_> {
    fn func(&mut self, id: FuncId, func: &Func) {
        // The code of a value takes the value before the parameters.
        let first = u32::from(func.value.is_some());
        let mut f = FuncState::new(first + func.params);
        let site = func.frame.then(|| f.local(ValType::I32));
        let mut vars = Vec::new();
        for (i, var) in func.vars.iter().enumerate() {
            let value = match u32::try_from(i) {
                Ok(param) if param < func.params => f.param(first + param),
                _ => f.local(
                    self.reprs
                        .local_type(var.rep)
                        .expect("a variable has a value"),
                ),
            };
            let defined = var.flagged.then(|| f.local(ValType::I32));
            let cell = var.cell.then(|| {
                let ty = self.reprs.cell_type(var.rep);
                let cell = f.local(ty.ref_to(true));
                f.ins().struct_new_default(ty).local_set(cell);
                cell
            });
            vars.push(Slots {
                value,
                cell,
                defined,
            });
        }
        // A parameter has a value from the start.
        for (slots, var) in vars.iter().zip(&func.vars).take(func.params as usize) {
            if let Some(defined) = slots.defined {
                f.ins().i32_const(1).local_set(defined);
            }
            if let Some(cell) = slots.cell {
                self.set_cell(&mut f, cell, var.rep, slots.value);
            }
        }
        let env = self.closure_of(id).map(|closure| {
            let ty = self.reprs.closure_type(closure);
            let env = f.local(ty.ref_to(true));
            let value = f.param(0);
            f.ins()
                .local_get(value)
                .ref_cast(ty.heap_type(), false)
                .local_set(env);
            (env, ty)
        });
        self.code = Code {
            func: Some(id),
            site,
            loops: Vec::new(),
            vars,
            target: None,
            chain: None,
            logic: None,
            env,
        };
        let index = self.func_slots(id).index;
        let Some(site) = site else {
            self.body(&mut f, &func.body);
            // Every path of the body of a function with a result ends in a
            // `return` or a `raise`.
            if func.ret != Rep::None {
                f.ins().unreachable();
            }
            self.m.define(index, f.finish());
            return;
        };
        self.reprs.enter(&mut f);
        self.reprs.begin(&mut f);
        self.body(&mut f, &func.body);
        if func.ret == Rep::None {
            self.stmt(&mut f, &Stmt::Return(None));
        }
        self.reprs.end_function(&mut f, site);
        self.m.define_frame(index, f, &[]);
    }

    fn stmt(&mut self, f: &mut FuncState, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => {
                self.expr(f, expr);
                if expr.rep() != Rep::None {
                    f.ins().drop();
                }
            }
            Stmt::Assign { targets, value } => {
                self.expr(f, value);
                let from = value.rep();
                let temp = match (targets.len(), self.reprs.local_type(from)) {
                    (2.., Some(ty)) => Some(f.temp(ty)),
                    _ => None,
                };
                if let Some(temp) = temp {
                    f.ins().local_set(temp);
                }
                for place in targets {
                    if let Some(temp) = temp {
                        self.reprs.load(f, from, temp);
                    }
                    let to = self.var(*place).rep;
                    if to != from {
                        self.reprs.widen(&mut self.m, f, from, to);
                    }
                    self.store(f, *place);
                }
                if let Some(temp) = temp {
                    f.free(temp);
                }
            }
            Stmt::Copy { from, to } => {
                let (from_rep, to_rep) = (self.var(Place::Global(*from)).rep, self.var(*to).rep);
                self.bound(f, Binding::Global(*from));
                f.if_(BlockType::Empty);
                self.load(f, Place::Global(*from), from_rep, None);
                if to_rep != from_rep {
                    self.reprs.widen(&mut self.m, f, from_rep, to_rep);
                }
                self.store(f, *to);
                f.end();
            }
            Stmt::Def(func) => {
                let defined = self.defined(*func);
                let ordered = self.program_func(*func).ordered;
                self.define_global(f, defined, ordered);
            }
            Stmt::Import(ids) => {
                for id in ids {
                    let defined = self.import_slot(*id);
                    let ordered = self
                        .program
                        .imports
                        .get(id.index())
                        .expect("an import of the program")
                        .ordered;
                    self.define_global(f, defined, ordered);
                }
            }
            Stmt::Class(id) => {
                let defined = self.class_slot(*id);
                let ordered = self
                    .program
                    .classes
                    .get(id.index())
                    .expect("a class of the program")
                    .ordered;
                self.define_global(f, defined, ordered);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    self.expr(f, value);
                }
                if self.code.site.is_some() {
                    self.reprs.leave(f);
                }
                f.ins().return_();
            }
            Stmt::If { cond, then, else_ } => {
                self.expr(f, cond);
                f.if_(BlockType::Empty);
                self.body(f, then);
                if !else_.is_empty() {
                    f.else_();
                    self.body(f, else_);
                }
                f.end();
            }
            Stmt::While { cond, body, else_ } => self.while_(f, cond, body, else_),
            Stmt::ForRange {
                var,
                start,
                stop,
                step,
                body,
                else_,
                site,
            } => self.for_range(f, *var, [start, stop, step], (body, else_), *site),
            Stmt::Unreachable => {
                f.ins().unreachable();
            }
            Stmt::Break => {
                let target = self.code.loops.last().expect("a break in a loop").break_;
                f.br(target);
            }
            Stmt::Example(body) => {
                let capture = self.host.capture.expect("a program that runs doctests");
                let done = self.reprs.begin_example(f, &capture);
                self.body(f, body);
                self.reprs.end_example(&mut self.m, f, &capture, done);
            }
            Stmt::Exit(code) => {
                self.expr(f, code);
                self.reprs.exit(f);
            }
            Stmt::SetField {
                object,
                class,
                field,
                value,
            } => {
                self.expr(f, value);
                let ty = self
                    .reprs
                    .local_type(value.rep())
                    .expect("a field has a value");
                let temp = f.temp(ty);
                f.ins().local_set(temp);
                self.expr(f, object);
                self.reprs.load(f, value.rep(), temp);
                f.free(temp);
                self.reprs.set_field(f, *class, *field);
            }
            Stmt::UpdateField {
                object,
                class,
                field,
                value,
            } => {
                self.expr(f, object);
                let ty = self
                    .reprs
                    .local_type(object.rep())
                    .expect("an object has a value");
                let target = f.temp(ty);
                f.ins().local_set(target);
                let outer = self.code.target.replace(target);
                f.ins().local_get(target);
                self.expr(f, value);
                self.code.target = outer;
                f.free(target);
                self.reprs.set_field(f, *class, *field);
            }
            Stmt::ListSet {
                list,
                index,
                value,
                site,
            } => {
                self.expr(f, value);
                let rep = value.rep();
                let item = f.temp(self.reprs.local_type(rep).expect("an item has a value"));
                f.ins().local_set(item);
                self.expr(f, list);
                self.expr(f, index);
                let list = list.rep();
                self.at(f, *site, |e, f| e.reprs.list_set(&mut e.m, f, list, item));
                f.free(item);
            }
            Stmt::StrSet {
                builder,
                index,
                value,
            } => {
                self.expr(f, builder);
                self.expr(f, index);
                self.expr(f, value);
                self.reprs.str_set(f);
            }
            Stmt::Raise { class, arg, site } => {
                if let Some(arg) = arg {
                    self.expr(f, arg);
                }
                self.at(f, *site, |e, f| match arg {
                    Some(_) => e.reprs.raise_text(&mut e.m, f, *class),
                    None => e.reprs.raise(&mut e.m, f, &Message::new(*class, "")),
                });
            }
            Stmt::Continue => {
                let target = self
                    .code
                    .loops
                    .last()
                    .expect("a continue in a loop")
                    .continue_;
                f.br(target);
            }
        }
    }

    fn body(&mut self, f: &mut FuncState, body: &[Stmt]) {
        for stmt in body {
            self.stmt(f, stmt);
        }
    }

    /// The `else` of the loop runs when the condition is false, after the
    /// loop and inside the block that `break` leaves.
    fn while_(&mut self, f: &mut FuncState, cond: &Expr, body: &[Stmt], else_: &[Stmt]) {
        let break_ = f.block(BlockType::Empty);
        let done = f.block(BlockType::Empty);
        let continue_ = f.loop_(BlockType::Empty);
        self.expr(f, cond);
        f.ins().i32_eqz();
        f.br_if(done);
        self.code.loops.push(Loop { break_, continue_ });
        self.body(f, body);
        self.code.loops.pop();
        f.br(continue_);
        f.end();
        f.end();
        self.body(f, else_);
        f.end();
    }

    /// The counter of the loop sums the step in an `i32` without a test,
    /// since `start` and `step` fit in 31 bits and so the sum fits in 32.
    fn for_range(
        &mut self,
        f: &mut FuncState,
        var: Place,
        [start, stop, step]: [&Expr; 3],
        (body, else_): (&[Stmt], &[Stmt]),
        site: Site,
    ) {
        let counter = f.temp(ValType::I32);
        let end = f.temp(ValType::I32);
        let by = f.temp(ValType::I32);
        self.expr(f, start);
        f.ins().local_set(counter);
        self.expr(f, stop);
        f.ins().local_set(end);
        self.expr(f, step);
        f.ins().local_set(by);
        // The test knows the sign of a literal step that is not 0.
        let sign = match step {
            Expr::Int(step) if *step != 0 => Some(*step > 0),
            _ => None,
        };
        if sign.is_none() {
            f.ins().local_get(by);
            self.at(f, site, |e, f| e.raise_if_zero(f, &RANGE_STEP_ZERO));
        }
        let break_ = f.block(BlockType::Empty);
        let done = f.block(BlockType::Empty);
        let top = f.loop_(BlockType::Empty);
        f.ins().local_get(counter).local_get(end);
        match sign {
            Some(true) => {
                f.ins().i32_lt_s();
            }
            Some(false) => {
                f.ins().i32_gt_s();
            }
            None => {
                f.ins()
                    .i32_lt_s()
                    .local_get(counter)
                    .local_get(end)
                    .i32_gt_s()
                    .local_get(by)
                    .i32_const(0)
                    .i32_gt_s()
                    .select();
            }
        }
        f.ins().i32_eqz();
        f.br_if(done);
        f.ins().local_get(counter);
        let rep = self.var(var).rep;
        if rep != Rep::Int {
            self.reprs.widen(&mut self.m, f, Rep::Int, rep);
        }
        self.store(f, var);
        let continue_ = f.block(BlockType::Empty);
        self.code.loops.push(Loop { break_, continue_ });
        self.body(f, body);
        self.code.loops.pop();
        f.end();
        f.ins()
            .local_get(counter)
            .local_get(by)
            .i32_add()
            .local_set(counter);
        f.br(top);
        f.end();
        f.end();
        self.body(f, else_);
        f.end();
        f.free(counter);
        f.free(end);
        f.free(by);
    }

    /// Pops a value into `place`.
    fn store(&mut self, f: &mut FuncState, place: Place) {
        match place {
            Place::Global(var) => {
                let slots = self.global_slots(var);
                let (value, defined) = (slots.value, slots.defined);
                f.ins().global_set(value);
                if let Some(defined) = defined {
                    let ordered = self.var(place).ordered;
                    self.define_global(f, defined, ordered);
                }
            }
            Place::Local(var) => {
                let slots = self.local_slots(var);
                let (value, cell, defined) = (slots.value, slots.cell, slots.defined);
                match cell {
                    Some(cell) => {
                        let rep = self.var(place).rep;
                        let temp =
                            f.temp(self.reprs.local_type(rep).expect("a variable has a value"));
                        f.ins().local_set(temp);
                        self.set_cell(f, cell, rep, temp);
                        f.free(temp);
                    }
                    None => {
                        f.ins().local_set(value);
                    }
                }
                if let Some(defined) = defined {
                    f.ins().i32_const(1).local_set(defined);
                }
            }
        }
    }

    /// Sets the flag `defined` of a global that gets a value. The flag of
    /// an ordered global keeps the place of its first binding.
    fn define_global(&mut self, f: &mut FuncState, defined: GlobalIdx, ordered: bool) {
        if !ordered {
            f.ins().i32_const(1).global_set(defined);
            return;
        }
        let order = self.order.expect("an ordered global has an order");
        f.ins().global_get(defined).i32_eqz();
        f.if_(BlockType::Empty);
        f.ins()
            .global_get(order)
            .i32_const(1)
            .i32_add()
            .global_set(order)
            .global_get(order)
            .global_set(defined);
        f.end();
    }

    fn load(&mut self, f: &mut FuncState, place: Place, rep: Rep, check: Option<&Check>) {
        if let Some(check) = check {
            self.bound(f, binding(place));
            self.raise_if_undefined(f, check);
        }
        match place {
            Place::Global(var) => self.reprs.load_global(f, rep, self.global_slots(var).value),
            Place::Local(var) => {
                let slots = self.local_slots(var);
                match slots.cell {
                    Some(cell) => {
                        f.ins()
                            .local_get(cell)
                            .struct_get(self.reprs.cell_type(rep), 0);
                        self.reprs.non_null(f, rep);
                    }
                    None => self.reprs.load(f, rep, slots.value),
                }
            }
        }
    }

    /// The place of `func` among the functions that capture variables, and
    /// `None` for a function that captures none.
    fn closure_of(&self, func: FuncId) -> Option<usize> {
        *self
            .closure_of
            .get(func.index())
            .expect("a function of the program")
    }

    /// Pushes the capture at `index` of the code that runs, a value of
    /// `rep`. A read of a cell without a value raises the error of `check`.
    fn capture(&mut self, f: &mut FuncState, index: u32, rep: Rep, check: Option<&Check>) {
        self.capture_field(f, index);
        if self.capture_of(index).cell {
            let ty = self.reprs.cell_type(rep);
            if let Some(check) = check {
                let temp = f.temp(ty.ref_to(true));
                f.ins().local_tee(temp).struct_get(ty, 1);
                self.raise_if_undefined(f, check);
                f.ins().local_get(temp);
                f.free(temp);
            }
            f.ins().struct_get(ty, 0);
        }
        self.reprs.non_null(f, rep);
    }

    /// The capture at `index` of the code that runs.
    fn capture_of(&self, index: u32) -> Capture {
        let func = self.code.func.expect("a capture in a function");
        *self
            .program
            .funcs
            .get(func.index())
            .and_then(|func| func.captures.get(index as usize))
            .expect("a capture of the code")
    }

    /// Pushes the field of the capture at `index` of the code that runs,
    /// which is a value or a cell.
    fn capture_field(&self, f: &mut FuncState, index: u32) {
        let (env, ty) = self.code.env.expect("the value of the code");
        f.ins().local_get(env).struct_get(ty, index + 1);
    }

    /// Pops the flag of a name, and raises the error of `check` when it is
    /// 0.
    fn raise_if_undefined(&mut self, f: &mut FuncState, check: &Check) {
        f.ins().i32_eqz();
        f.if_(BlockType::Empty);
        self.at(f, check.site, |e, f| match &check.error {
            Undefined::Local(text) => {
                let message = Message::new(Class::UnboundLocalError, text);
                e.reprs.raise(&mut e.m, f, &message);
            }
            Undefined::Global(error) => {
                e.name_error_text(f, error);
                e.reprs.raise_text(&mut e.m, f, Class::NameError);
            }
        });
        f.end();
    }

    /// Emits the code of `emit` as code that runs at `site`. An exception
    /// gets the frame of the site, and so does a trap, which wasmtime gives
    /// when the program runs out of memory.
    fn at(&mut self, f: &mut FuncState, site: Site, emit: impl FnOnce(&mut Self, &mut FuncState)) {
        // An exception of a function without a frame has the site of the
        // call of the function.
        let Some(local) = self.code.site else {
            emit(self, f);
            return;
        };
        f.ins().i32_const(site_index(site)).local_set(local);
        let start = f.position();
        emit(self, f);
        f.mark(start, site);
    }

    /// Pushes the text of `error`, with the suggestion of CPython for what
    /// the frame has now.
    fn name_error_text(&mut self, f: &mut FuncState, error: &NameError) {
        if error.groups.is_empty() && error.self_attr.is_none() {
            self.reprs.str_literal(&mut self.m, f, &error.text);
            return;
        }
        let str_type = self.reprs.local_type(Rep::Str).expect("a str has a value");
        let ValType::Ref(str_ref) = str_type else {
            unreachable!("a str is a reference");
        };
        let found = f.temp(str_type);
        f.ins().ref_null(str_ref.heap_type).local_set(found);
        let suggested = f.block(BlockType::Empty);
        if let Some((binding, candidate)) = &error.self_attr {
            self.suggest_if_bound(f, *binding, candidate, found, suggested);
        }
        if let Some(limit) = &error.limit {
            f.ins().i32_const(count(limit.always));
            if let Some(order) = self.order {
                f.ins()
                    .global_get(order)
                    .i32_const(count(limit.globals))
                    .i32_mul()
                    .i32_add();
            }
            for local in &limit.locals {
                self.bound(f, *local);
                f.ins().i32_add();
            }
            f.ins().i32_const(count(limit.max)).i32_ge_u();
            f.br_if(suggested);
        }
        for group in &error.groups {
            for (local, candidate) in &group.locals {
                self.suggest_if_bound(f, *local, candidate, found, suggested);
            }
            if let [(global, candidate)] = group.globals.as_slice() {
                self.suggest_if_bound(f, *global, candidate, found, suggested);
            } else if !group.globals.is_empty() {
                // The global with the least place wins. A global without a
                // value has 0, whose place less 1 is the greatest.
                let best = f.temp(ValType::I32);
                let place = f.temp(ValType::I32);
                f.ins().i32_const(0).local_set(best);
                for (global, candidate) in &group.globals {
                    self.bound(f, *global);
                    f.ins()
                        .local_tee(place)
                        .i32_const(1)
                        .i32_sub()
                        .local_get(best)
                        .i32_const(1)
                        .i32_sub()
                        .i32_lt_u();
                    f.if_(BlockType::Empty);
                    f.ins().local_get(place).local_set(best);
                    self.reprs.str_literal(&mut self.m, f, candidate);
                    f.ins().local_set(found);
                    f.end();
                }
                f.ins().local_get(best);
                f.br_if(suggested);
                f.free(best);
                f.free(place);
            }
            if let Some(candidate) = &group.always {
                self.reprs.str_literal(&mut self.m, f, candidate);
                f.ins().local_set(found);
                f.br(suggested);
            }
        }
        f.end();

        let str_value = self.reprs.value_type(Rep::Str).expect("a str has a value");
        f.ins().local_get(found).ref_is_null();
        f.if_(BlockType::Result(str_value));
        self.reprs.str_literal(&mut self.m, f, &error.text);
        f.else_();
        let (before, after) = &error.suggested;
        self.reprs.str_literal(&mut self.m, f, before);
        self.reprs.load(f, Rep::Str, found);
        self.reprs.str_concat(f);
        self.reprs.str_literal(&mut self.m, f, after);
        self.reprs.str_concat(f);
        f.end();
        f.free(found);
    }

    /// Sets `found` to `candidate` and leaves `suggested` when `binding`
    /// has a value.
    fn suggest_if_bound(
        &mut self,
        f: &mut FuncState,
        binding: Binding,
        candidate: &str,
        found: LocalIdx,
        suggested: Label,
    ) {
        self.bound(f, binding);
        f.if_(BlockType::Empty);
        self.reprs.str_literal(&mut self.m, f, candidate);
        f.ins().local_set(found);
        f.br(suggested);
        f.end();
    }

    /// Stores the value of the local `value` in `cell`, a cell of a variable
    /// of `rep`, which then has a value.
    fn set_cell(&self, f: &mut FuncState, cell: LocalIdx, rep: Rep, value: LocalIdx) {
        let ty = self.reprs.cell_type(rep);
        f.ins()
            .local_get(cell)
            .local_get(value)
            .struct_set(ty, 0)
            .local_get(cell)
            .i32_const(1)
            .struct_set(ty, 1);
    }

    /// Pushes an `i32` that is not 0 when `binding` has a value. The flag of
    /// a local is 1.
    fn bound(&self, f: &mut FuncState, binding: Binding) {
        let global = match binding {
            Binding::Local(var) => {
                match self.local_slots(var).defined {
                    Some(defined) => f.ins().local_get(defined),
                    // A parameter has a value from the start.
                    None => f.ins().i32_const(1),
                };
                return;
            }
            // A cell tells whether its variable has a value, and a capture
            // without one has a value.
            Binding::Capture(index) => {
                let capture = self.capture_of(index);
                if capture.cell {
                    self.capture_field(f, index);
                    f.ins().struct_get(self.reprs.cell_type(capture.rep), 1);
                } else {
                    f.ins().i32_const(1);
                }
                return;
            }
            Binding::Global(var) => self.global_slots(var).defined.expect("a flagged global"),
            Binding::Func(func) => self.defined(func),
            Binding::Import(id) => self.import_slot(id),
            Binding::Class(id) => self.class_slot(id),
        };
        f.ins().global_get(global);
    }

    /// Pops an `i32`, and raises an exception with `message` when it is 0.
    fn raise_if_zero(&mut self, f: &mut FuncState, message: &Message) {
        f.ins().i32_eqz();
        f.if_(BlockType::Empty);
        self.reprs.raise(&mut self.m, f, message);
        f.end();
    }

    fn call(
        &mut self,
        f: &mut FuncState,
        func: FuncId,
        (args, order): (&[Expr], Option<&[u32]>),
        site: Option<Site>,
        check: Option<&Check>,
    ) {
        if let Some(check) = check {
            self.bound(f, Binding::Func(func));
            self.raise_if_undefined(f, check);
        }
        if let Some(order) = order {
            // The program evaluates the arguments in the order of the call.
            let mut values = Vec::new();
            for arg in args {
                self.expr(f, arg);
                let rep = arg.rep();
                let local = f.temp(self.reprs.local_type(rep).expect("an argument has a value"));
                f.ins().local_set(local);
                values.push((rep, local));
            }
            for arg in order {
                let (rep, local) = *values.get(*arg as usize).expect("an argument of the call");
                self.reprs.load(f, rep, local);
            }
            for (_, local) in values {
                f.free(local);
            }
        } else {
            for arg in args {
                self.expr(f, arg);
            }
        }
        self.invoke(f, func, site);
    }

    /// The call of `func` on the arguments that the stack holds.
    fn invoke(&mut self, f: &mut FuncState, func: FuncId, site: Option<Site>) {
        let index = self.func_slots(func).index;
        let Some(site) = site else {
            f.ins().call(index);
            return;
        };
        let frame = self.program_func(func).frame;
        self.at(f, site, |e, f| {
            // A function without a frame does not count against the limit
            // of the recursion.
            if frame {
                e.reprs.call_fits(f);
                e.raise_if_zero(f, &RECURSION_ERROR);
            }
            f.ins().call(index);
        });
    }

    /// The call of the value of `callee` with `args`, through the code that
    /// the value holds, which takes the value before the arguments.
    fn call_value(
        &mut self,
        f: &mut FuncState,
        callee: &Expr,
        args: &[Expr],
        (site, limit): (Site, Site),
    ) {
        let Rep::Func(ty) = callee.rep() else {
            unreachable!("a call of a value that is not a function");
        };
        self.expr(f, callee);
        let value = f.temp(
            self.reprs
                .local_type(callee.rep())
                .expect("a function has a value"),
        );
        f.ins().local_tee(value);
        for arg in args {
            self.expr(f, arg);
        }
        f.ins()
            .local_get(value)
            .struct_get(self.reprs.func_value(ty), 0);
        f.free(value);
        let code = self.reprs.func_code(ty);
        if site == limit {
            self.at(f, site, |e, f| {
                e.reprs.call_fits(f);
                e.raise_if_zero(f, &RECURSION_ERROR);
                f.ins().call_ref(code);
            });
            return;
        }
        self.at(f, limit, |e, f| {
            e.reprs.call_fits(f);
            e.raise_if_zero(f, &RECURSION_ERROR);
        });
        self.at(f, site, |_, f| {
            f.ins().call_ref(code);
        });
    }

    /// The global that is not 0 once the `def` of `func` has run.
    fn defined(&self, func: FuncId) -> GlobalIdx {
        self.func_slots(func)
            .defined
            .expect("a function that a def binds")
    }

    fn global_slots(&self, var: VarId) -> &Slots<GlobalIdx> {
        self.vars
            .get(var.index())
            .expect("a variable of the module")
    }

    fn local_slots(&self, var: VarId) -> &Slots<LocalIdx> {
        self.code
            .vars
            .get(var.index())
            .expect("a variable of the function")
    }

    fn var(&self, place: Place) -> &crate::ir::Var {
        match place {
            Place::Global(var) => self.program.vars.get(var.index()),
            Place::Local(var) => {
                let func = self.code.func.expect("a local in a function");
                self.program_func(func).vars.get(var.index())
            }
        }
        .expect("a variable of the program")
    }

    fn func_slots(&self, func: FuncId) -> &FuncSlots {
        self.funcs
            .get(func.index())
            .expect("a function of the program")
    }

    fn import_slot(&self, id: ImportId) -> GlobalIdx {
        *self
            .imports
            .get(id.index())
            .expect("an import of the program")
    }

    fn class_slot(&self, id: ClassId) -> GlobalIdx {
        *self
            .classes
            .get(id.index())
            .expect("a class of the program")
    }

    /// The function that a call of `func` runs, which is the one that
    /// `func` names as its alias, when it has one.
    fn program_func(&self, func: FuncId) -> &Func {
        let get = |func: FuncId| {
            self.program
                .funcs
                .get(func.index())
                .expect("a function of the program")
        };
        let found = get(func);
        found.alias.map_or(found, get)
    }

    /// Pushes the value of `expr`, and nothing when its rep is `None`.
    fn expr(&mut self, f: &mut FuncState, expr: &Expr) {
        match expr {
            Expr::Str(s) => self.reprs.str_literal(&mut self.m, f, s),
            Expr::Int(value) => self.reprs.int_literal(f, *value),
            Expr::Float(value) => self.reprs.float_literal(f, *value),
            Expr::Bool(value) => self.reprs.bool_literal(f, *value),
            Expr::None => {}
            Expr::Widen { value, rep } => {
                self.expr(f, value);
                self.reprs.widen(&mut self.m, f, value.rep(), *rep);
            }
            Expr::Narrow { value, rep } => {
                self.expr(f, value);
                self.reprs.narrow(&mut self.m, f, value.rep(), *rep);
            }
            Expr::IsNone(value) => {
                self.expr(f, value);
                self.reprs.is_none(f, value.rep());
            }
            Expr::Is(left, right) => {
                self.expr(f, left);
                self.expr(f, right);
                f.ins().ref_eq();
            }
            Expr::Load { place, rep, check } => self.load(f, *place, *rep, check.as_ref()),
            Expr::Call {
                func,
                args,
                order,
                site,
                check,
                ..
            } => self.call(f, *func, (args, order.as_deref()), *site, check.as_ref()),
            Expr::BoolAsInt(operand) => {
                self.expr(f, operand);
                self.reprs.bool_as_int(f);
            }
            Expr::CallValue {
                callee,
                args,
                site,
                limit,
                ..
            } => self.call_value(f, callee, args, (*site, *limit)),
            Expr::Closure {
                func,
                ty,
                captures: Some(captures),
            } => {
                let index = self.func_slots(*func).index;
                self.m.declare_func(index);
                f.ins().ref_func(index);
                for capture in captures {
                    match capture {
                        Captured::Value(value) => self.expr(f, value),
                        Captured::Cell(var) => {
                            let cell = self.local_slots(*var).cell.expect("a variable in a cell");
                            f.ins().local_get(cell);
                        }
                        Captured::Outer(index) => self.capture_field(f, *index),
                    }
                }
                let ty = match self.closure_of(*func) {
                    Some(closure) => self.reprs.closure_type(closure),
                    None => self.reprs.func_value(*ty),
                };
                f.ins().struct_new(ty);
            }
            Expr::Capture { index, rep, check } => {
                self.capture(f, *index, *rep, check.as_ref());
            }
            Expr::Closure {
                func,
                ty,
                captures: None,
            } => {
                let global = match self.values.get(func) {
                    Some(global) => *global,
                    None => {
                        let index = self.func_slots(*func).index;
                        let global = self.m.func_global(self.reprs.func_value(*ty), index);
                        self.values.insert(*func, global);
                        global
                    }
                };
                f.ins().global_get(global);
            }
            Expr::Arith {
                op,
                left,
                right,
                site,
            } => {
                self.expr(f, left);
                self.expr(f, right);
                self.at(f, *site, |e, f| e.reprs.int_arith(f, *op));
            }
            Expr::Bits {
                op,
                left,
                right,
                rep: _,
            } => {
                self.expr(f, left);
                self.expr(f, right);
                self.reprs.int_bits(f, *op);
            }
            Expr::IntAsFloat(operand) => {
                self.expr(f, operand);
                self.reprs.int_as_float(f);
            }
            Expr::FloatArith { op, left, right } => {
                self.expr(f, left);
                self.expr(f, right);
                self.reprs.float_arith(f, *op);
            }
            Expr::FloatNeg(operand) => {
                self.expr(f, operand);
                self.reprs.float_neg(f);
            }
            Expr::IntAbs { operand, site } => {
                self.expr(f, operand);
                self.at(f, *site, |e, f| e.reprs.int_abs(f));
            }
            Expr::IntRound {
                operand,
                digits,
                site,
            } => {
                self.expr(f, operand);
                self.expr(f, digits);
                self.at(f, *site, |e, f| e.reprs.int_round(&mut e.m, f));
            }
            Expr::FloatAsInt { operand, site } => {
                self.expr(f, operand);
                self.at(f, *site, |e, f| e.reprs.float_as_int(&mut e.m, f));
            }
            Expr::FloatAbs(operand) => {
                self.expr(f, operand);
                self.reprs.float_abs(f);
            }
            Expr::FloatRoundDigits {
                operand,
                digits,
                site,
            } => {
                self.expr(f, operand);
                self.expr(f, digits);
                self.at(f, *site, |e, f| e.reprs.float_round_digits(&mut e.m, f));
            }
            Expr::FloatRound(operand) => {
                self.expr(f, operand);
                self.reprs.float_round(f);
            }
            Expr::After {
                binding,
                check,
                value,
            } => {
                self.bound(f, *binding);
                self.raise_if_undefined(f, check);
                self.expr(f, value);
            }
            Expr::NotNone { value, text, site } => {
                self.expr(f, value);
                let message = Message::new(Class::AttributeError, text);
                self.check_value(f, value.rep(), *site, &message, |_, f| {
                    f.ins().ref_is_null();
                });
            }
            Expr::Expect {
                value,
                rep,
                text,
                site,
            } => {
                self.expr(f, value);
                let from = value.rep();
                let message = Message::new(Class::TypeError, text);
                self.check_value(f, from, *site, &message, |e, f| {
                    e.reprs.has(&mut e.m, f, from, *rep);
                    f.ins().i32_eqz();
                });
                self.reprs.narrow(&mut self.m, f, from, *rep);
            }
            Expr::MathCall {
                func,
                operand,
                site,
            } => {
                self.expr(f, operand);
                self.at(f, *site, |e, f| e.reprs.math_call(&mut e.m, f, *func));
            }
            Expr::Math2Call { func, x, y, site } => {
                self.expr(f, x);
                self.expr(f, y);
                self.at(f, *site, |e, f| e.reprs.math2_call(&mut e.m, f, *func));
            }
            Expr::Fma { x, y, z, site } => {
                self.expr(f, x);
                self.expr(f, y);
                self.expr(f, z);
                self.at(f, *site, |e, f| e.reprs.fma(&mut e.m, f));
            }
            Expr::Float2Call { func, x, y } => {
                self.expr(f, x);
                self.expr(f, y);
                self.reprs.float2_call(&mut self.m, f, *func);
            }
            Expr::Hypot(args) => {
                for arg in args {
                    self.expr(f, arg);
                }
                self.reprs.hypot(&mut self.m, f, args.len());
            }
            Expr::Ldexp { x, i, site } => {
                self.expr(f, x);
                self.expr(f, i);
                self.at(f, *site, |e, f| e.reprs.ldexp(&mut e.m, f));
            }
            Expr::LogBase {
                operand,
                base,
                site,
            } => {
                self.expr(f, operand);
                self.expr(f, base);
                self.at(f, *site, |e, f| e.reprs.log_base(&mut e.m, f));
            }
            Expr::IntGcd { args, site } => {
                self.reprs.gcd_start(f);
                for arg in args {
                    self.expr(f, arg);
                    self.reprs.gcd_step(&mut self.m, f);
                }
                self.at(f, *site, |e, f| e.reprs.gcd_end(f));
            }
            Expr::IntLcm { args, site } => {
                self.reprs.lcm_start(f);
                for arg in args {
                    self.expr(f, arg);
                    self.reprs.lcm_step(&mut self.m, f);
                }
                self.at(f, *site, |e, f| e.reprs.lcm_end(&mut e.m, f));
            }
            Expr::IntPow { base, exp, site } => {
                self.expr(f, base);
                self.expr(f, exp);
                self.at(f, *site, |e, f| e.reprs.int_pow(&mut e.m, f));
            }
            Expr::IntChoose { choice, n, k, site } => {
                self.expr(f, n);
                self.expr(f, k);
                self.at(f, *site, |e, f| e.reprs.choose(&mut e.m, f, *choice));
            }
            Expr::Fmod { x, y, site } => {
                self.expr(f, x);
                self.expr(f, y);
                self.at(f, *site, |e, f| e.reprs.fmod(&mut e.m, f));
            }
            Expr::IsClose {
                a,
                b,
                tolerances,
                site,
            } => {
                self.expr(f, a);
                self.expr(f, b);
                let val_type = self
                    .reprs
                    .local_type(Rep::Float)
                    .expect("a float has a value");
                let (rel, abs) = (f.temp(val_type), f.temp(val_type));
                self.reprs.float_literal(f, 1e-09);
                f.ins().local_set(rel);
                self.reprs.float_literal(f, 0.0);
                f.ins().local_set(abs);
                for (tolerance, value) in tolerances {
                    self.expr(f, value);
                    f.ins().local_set(match tolerance {
                        Tolerance::Rel => rel,
                        Tolerance::Abs => abs,
                    });
                }
                f.ins().local_get(rel).local_get(abs);
                f.free(rel);
                f.free(abs);
                self.at(f, *site, |e, f| e.reprs.isclose(&mut e.m, f));
            }
            Expr::FloatSqrt { operand, site } => {
                self.expr(f, operand);
                self.at(f, *site, |e, f| e.reprs.float_sqrt(&mut e.m, f));
            }
            Expr::FloatFloor(operand) => {
                self.expr(f, operand);
                self.reprs.float_floor(f);
            }
            Expr::FloatCeil(operand) => {
                self.expr(f, operand);
                self.reprs.float_ceil(f);
            }
            Expr::Div {
                op,
                left,
                right,
                site,
            } => {
                self.expr(f, left);
                self.expr(f, right);
                self.at(f, *site, |e, f| e.reprs.float_div(&mut e.m, f, *op));
            }
            Expr::FloatPow { left, right, site } => {
                self.expr(f, left);
                self.expr(f, right);
                self.at(f, *site, |e, f| e.reprs.float_pow(&mut e.m, f));
            }
            Expr::Concat { left, right, site } => {
                self.expr(f, left);
                self.expr(f, right);
                self.at(f, *site, |e, f| e.reprs.str_concat(f));
            }
            Expr::StrOf { operand, site } => {
                self.expr(f, operand);
                let rep = operand.rep();
                self.at(f, *site, |e, f| e.reprs.str_of(&mut e.m, f, rep));
            }
            Expr::Join { parts, site } => {
                for part in parts {
                    self.expr(f, part);
                }
                self.at(f, *site, |e, f| {
                    for _ in 1..parts.len() {
                        e.reprs.str_concat(f);
                    }
                });
            }
            Expr::Repeat { left, right, site } => {
                self.expr(f, left);
                self.expr(f, right);
                if left.rep() != Rep::Str {
                    let s = f.temp(self.reprs.local_type(Rep::Str).expect("a str has a value"));
                    let n = f.temp(self.reprs.local_type(Rep::Int).expect("an int has a value"));
                    f.ins().local_set(s).local_set(n);
                    self.reprs.load(f, Rep::Str, s);
                    self.reprs.load(f, Rep::Int, n);
                    f.free(s);
                    f.free(n);
                }
                self.at(f, *site, |e, f| e.reprs.str_repeat(&mut e.m, f));
            }
            Expr::UniformRepr { value, site } => {
                self.expr(f, value);
                self.at(f, *site, |e, f| e.reprs.uniform_repr(&mut e.m, f));
            }
            Expr::UniformEq {
                left,
                right,
                eqs,
                site,
            } => {
                self.expr(f, left);
                self.expr(f, right);
                let eqs: Vec<_> = eqs
                    .iter()
                    .map(|(class, func)| (*class, self.func_slots(*func).index))
                    .collect();
                self.at(f, *site, |e, f| e.reprs.uniform_eq(&mut e.m, f, eqs));
            }
            Expr::Captured => self.reprs.captured(f),
            Expr::CapturedException => self.reprs.captured_exception(f),
            Expr::Target(_) => {
                let target = self.code.target.expect("a target in an update of a field");
                f.ins().local_get(target);
            }
            Expr::Stop { text, site } => {
                self.expr(f, text);
                self.at(f, *site, |e, f| {
                    e.reprs.raise_text(&mut e.m, f, Class::Limit)
                });
            }
            Expr::WriteErr(text) => {
                self.expr(f, text);
                f.ins().call(self.host.to_stderr);
                self.reprs.str_write(f);
                f.ins().call(self.host.to_stdout);
            }
            Expr::StrNew { len, site } => {
                self.expr(f, len);
                self.at(f, *site, |e, f| e.reprs.str_new(f));
            }
            Expr::ListCapacity(list) => {
                self.expr(f, list);
                self.reprs.list_capacity(f, list.rep());
            }
            Expr::ListReserve {
                list,
                capacity,
                site,
            } => {
                self.expr(f, list);
                self.expr(f, capacity);
                let rep = list.rep();
                self.at(f, *site, |e, f| e.reprs.list_reserve(f, rep));
            }
            Expr::ListSetLen { list, len } => {
                self.expr(f, list);
                self.expr(f, len);
                self.reprs.list_set_len(f, list.rep());
            }
            Expr::ListTruncate { list, len } => {
                self.expr(f, list);
                self.expr(f, len);
                self.reprs.list_truncate(f, list.rep());
            }
            Expr::ListIndex {
                list,
                index,
                rep,
                site,
            } => {
                self.expr(f, list);
                self.expr(f, index);
                let list = list.rep();
                self.at(f, *site, |e, f| e.reprs.list_index(&mut e.m, f, list, *rep));
            }
            Expr::Item { list, index, rep } => {
                self.expr(f, list);
                self.reprs.list_items(f, list.rep());
                self.expr(f, index);
                self.reprs.index_of(f);
                self.reprs.list_item(f, list.rep(), *rep);
            }
            Expr::SetItem { list, index, value } => {
                self.expr(f, list);
                self.reprs.list_items(f, list.rep());
                self.expr(f, index);
                self.reprs.index_of(f);
                self.expr(f, value);
                self.reprs.set_list_item(f, list.rep());
            }
            Expr::CopyItems {
                dst,
                at,
                src,
                start,
                count,
            } => {
                let rep = dst.rep();
                self.expr(f, dst);
                self.reprs.list_items(f, rep);
                self.expr(f, at);
                self.reprs.index_of(f);
                self.expr(f, src);
                self.reprs.list_items(f, rep);
                self.expr(f, start);
                self.reprs.index_of(f);
                self.expr(f, count);
                self.reprs.index_of(f);
                self.reprs.copy_list_items(f, rep);
            }
            Expr::CodePoint { value, index } => {
                self.expr(f, value);
                self.expr(f, index);
                self.reprs.code_point(f);
            }
            Expr::CaseAt {
                case,
                operand,
                index,
            } => {
                self.expr(f, operand);
                self.expr(f, index);
                self.reprs.case_at(&mut self.m, f, *case);
            }
            Expr::ParseFloat(text) => {
                self.expr(f, text);
                self.reprs.parse_float(&mut self.m, f);
            }
            Expr::Hash(value) => {
                self.expr(f, value);
                self.reprs.hash(&mut self.m, f, value.rep());
            }
            Expr::HashAsInt { value, site } => {
                self.expr(f, value);
                self.at(f, *site, |e, f| e.reprs.i64_as_int(f));
            }
            Expr::HashTuple(items) => {
                self.reprs.hash_tuple_start(f);
                for item in items {
                    self.expr(f, item);
                    self.reprs.hash_tuple_item(f);
                }
                self.reprs.hash_tuple_end(f, items.len());
            }
            Expr::HashSlot { hash, mask } => {
                self.expr(f, hash);
                self.expr(f, mask);
                self.reprs.hash_slot(f);
            }
            Expr::HashShift(hash) => {
                self.expr(f, hash);
                f.ins().i64_const(5).i64_shr_u();
            }
            Expr::HashEq(a, b) => {
                self.expr(f, a);
                self.expr(f, b);
                f.ins().i64_eq();
            }
            Expr::DecimalValue(operand) => {
                self.expr(f, operand);
                self.reprs.decimal_value(&mut self.m, f);
            }
            Expr::CodePointIs { set, operand } => {
                self.expr(f, operand);
                self.reprs.code_point_is(&mut self.m, f, *set);
            }
            Expr::Index { value, index, site } => {
                self.expr(f, value);
                self.expr(f, index);
                self.at(f, *site, |e, f| e.reprs.str_index(&mut e.m, f));
            }
            Expr::Slice {
                value,
                start,
                stop,
                site,
            } => {
                self.expr(f, value);
                match start {
                    Some(start) => self.expr(f, start),
                    None => self.reprs.int_literal(f, 0),
                }
                match stop {
                    Some(stop) => {
                        self.expr(f, stop);
                        self.reprs.slice_stop(f);
                    }
                    None => self.reprs.slice_to_the_end(f),
                }
                self.at(f, *site, |e, f| e.reprs.str_slice(&mut e.m, f));
            }
            Expr::Len { operand, site } => {
                self.expr(f, operand);
                let rep = operand.rep();
                self.at(f, *site, |e, f| e.reprs.len(f, rep));
            }
            Expr::Truth(operand) => {
                self.expr(f, operand);
                self.reprs.truth(&mut self.m, f, operand.rep());
            }
            Expr::Not(operand) => {
                self.expr(f, operand);
                self.reprs.bool_not(f);
            }
            Expr::Member { class, member } => self.reprs.member(f, *class, *member),
            Expr::InRepr(object) => {
                self.expr(f, object);
                self.reprs.in_repr(&mut self.m, f);
            }
            Expr::DuringRepr { object, value } => {
                self.expr(f, object);
                self.reprs.push_repr(f);
                self.expr(f, value);
                self.reprs.pop_repr(f);
            }
            Expr::New { class, fields } => {
                for field in fields {
                    self.expr(f, field);
                }
                self.reprs.new_object(f, *class);
            }
            Expr::Empty(class) => self.reprs.empty_object(f, *class),
            Expr::IsExact { value, class } => {
                self.expr(f, value);
                self.reprs.is_exact(f, *class);
            }
            Expr::IsInstance { value, rep, check } => {
                self.expr(f, value);
                if let Some((binding, check)) = check {
                    self.bound(f, *binding);
                    self.raise_if_undefined(f, check);
                }
                let from = value.rep();
                match rep {
                    Some(rep) if *rep != from => {
                        self.reprs.is_instance(&mut self.m, f, from, *rep);
                    }
                    _ => {
                        if from != Rep::None {
                            f.ins().drop();
                        }
                        f.ins().i32_const(i32::from(rep.is_some()));
                    }
                }
            }
            Expr::AsExact { value, class } => {
                self.expr(f, value);
                self.reprs.as_exact(f, *class);
            }
            Expr::List { list, items } => {
                let array = self.reprs.begin_literal(f, *list, items.len());
                for (i, item) in items.iter().enumerate() {
                    self.reprs.before_item(f, array, i);
                    self.expr(f, item);
                    self.reprs.after_item(f, *list, array);
                }
                self.reprs.end_literal(f, *list, items.len(), array);
            }
            Expr::Field {
                object,
                class,
                field,
                rep,
            } => {
                self.expr(f, object);
                self.reprs.field(f, *class, *field, *rep);
            }
            Expr::Logic {
                op,
                left,
                test,
                value,
                right,
            } => self.logic(f, *op, left, (test, value), right),
            Expr::LogicLeft(rep) => {
                if let Some(local) = self.code.logic {
                    self.reprs.load(f, *rep, local);
                }
            }
            Expr::Block { stmts, value } => {
                self.body(f, stmts);
                self.expr(f, value);
            }
            Expr::IfElse { cond, then, else_ } => {
                self.expr(f, cond);
                let ty = self.reprs.value_type(then.rep());
                f.if_(ty.map_or(BlockType::Empty, BlockType::Result));
                self.expr(f, then);
                f.else_();
                self.expr(f, else_);
                f.end();
            }
            Expr::Compare(first, rest) => self.compare(f, first, rest),
            Expr::Chain { first, links } => self.chain(f, first, links),
            Expr::ChainLeft(_) => {
                let (left, _) = self.code.chain.expect("a test of a chain");
                f.ins().local_get(left);
            }
            Expr::ChainRight(_) => {
                let (_, right) = self.code.chain.expect("a test of a chain");
                f.ins().local_get(right);
            }
            Expr::MinMax { op, args } => self.min_max(f, *op, args),
            Expr::Print { args, site } => self.print(f, args, *site),
        }
    }

    fn min_max(&mut self, f: &mut FuncState, op: MinMaxOp, args: &[Expr]) {
        let Some((first, rest)) = args.split_first() else {
            unreachable!("min or max without values");
        };
        let rep = first.rep();
        let ty = self.reprs.local_type(rep).expect("a value of min or max");
        let result = f.temp(ty);
        let value = f.temp(ty);
        self.expr(f, first);
        f.ins().local_set(result);
        for arg in rest {
            self.expr(f, arg);
            f.ins().local_set(value);
            self.reprs.load(f, rep, value);
            self.reprs.load(f, rep, result);
            let cmp = match op {
                MinMaxOp::Min => CmpOp::Lt,
                MinMaxOp::Max => CmpOp::Gt,
            };
            self.reprs.compare(&mut self.m, f, rep, cmp);
            f.if_(BlockType::Empty);
            f.ins().local_get(value).local_set(result);
            f.end();
        }
        self.reprs.load(f, rep, result);
        f.free(value);
        f.free(result);
    }

    /// The left operand goes to a local, which gives both its truth and,
    /// when the truth decides the result, the result.
    fn logic(
        &mut self,
        f: &mut FuncState,
        op: LogicOp,
        left: &Expr,
        (test, value): (&Expr, &Expr),
        right: &Expr,
    ) {
        let local = self.reprs.local_type(left.rep()).map(|ty| f.temp(ty));
        self.expr(f, left);
        if let Some(local) = local {
            f.ins().local_set(local);
        }
        let outer = self.code.logic;
        self.code.logic = local;
        self.expr(f, test);
        let rep = value.rep();
        f.if_(
            self.reprs
                .value_type(rep)
                .map_or(BlockType::Empty, BlockType::Result),
        );
        let (then, else_) = match op {
            LogicOp::And => (right, value),
            LogicOp::Or => (value, right),
        };
        self.expr(f, then);
        f.else_();
        self.expr(f, else_);
        f.end();
        self.code.logic = outer;
        if let Some(local) = local {
            f.free(local);
        }
    }

    /// A chain stops at the first comparison that is false, and the operands
    /// after it are not evaluated.
    fn compare(&mut self, f: &mut FuncState, first: &Expr, rest: &[(CmpOp, Expr)]) {
        let Some(((last_op, last), init)) = rest.split_last() else {
            unreachable!("a comparison without operators");
        };
        let rep = first.rep();
        let result = f.block(BlockType::Result(
            self.reprs
                .local_type(Rep::Bool)
                .expect("a bool has a value"),
        ));
        self.expr(f, first);
        if !init.is_empty() {
            let right = f.temp(self.reprs.local_type(rep).expect("an operand has a value"));
            for (op, operand) in init {
                self.expr(f, operand);
                f.ins().local_set(right);
                self.reprs.load(f, rep, right);
                self.reprs.compare(&mut self.m, f, rep, *op);
                f.ins().i32_eqz();
                f.if_(BlockType::Empty);
                self.reprs.bool_literal(f, false);
                f.br(result);
                f.end();
                self.reprs.load(f, rep, right);
            }
            f.free(right);
        }
        self.expr(f, last);
        self.reprs.compare(&mut self.m, f, rep, *last_op);
        f.end();
    }

    /// Each operand goes to a local, which the test of its link reads on
    /// the right and the test of the next link reads on the left.
    fn chain(&mut self, f: &mut FuncState, first: &Expr, links: &[(Expr, Expr)]) {
        let bool = self
            .reprs
            .local_type(Rep::Bool)
            .expect("a bool has a value");
        let local = |e: &Self, f: &mut FuncState, rep: Rep| {
            f.temp(e.reprs.local_type(rep).expect("an operand has a value"))
        };
        let result = f.block(BlockType::Result(bool));
        let mut left = local(self, f, first.rep());
        self.expr(f, first);
        f.ins().local_set(left);
        let outer = self.code.chain;
        for (i, (operand, test)) in links.iter().enumerate() {
            let right = local(self, f, operand.rep());
            self.expr(f, operand);
            f.ins().local_set(right);
            self.code.chain = Some((left, right));
            self.expr(f, test);
            self.code.chain = outer;
            if i + 1 < links.len() {
                f.ins().i32_eqz();
                f.if_(BlockType::Empty);
                self.reprs.bool_literal(f, false);
                f.br(result);
                f.end();
            }
            f.free(left);
            left = right;
        }
        f.free(left);
        f.end();
    }

    /// Keeps the value of `rep` on the stack, and raises `message` at
    /// `site` when `fails` pushes `true` for the value.
    fn check_value(
        &mut self,
        f: &mut FuncState,
        rep: Rep,
        site: Site,
        message: &Message,
        fails: impl FnOnce(&mut Self, &mut FuncState),
    ) {
        let ty = self.reprs.local_type(rep).expect("a checked value");
        let local = f.temp(ty);
        f.ins().local_tee(local);
        fails(self, f);
        f.if_(BlockType::Empty);
        self.at(f, site, |e, f| e.reprs.raise(&mut e.m, f, message));
        f.end();
        f.ins().local_get(local);
        f.free(local);
    }

    /// Python evaluates every argument before it writes the first one.
    fn print(&mut self, f: &mut FuncState, args: &[Expr], site: Site) {
        let mut values = Vec::new();
        for arg in args {
            self.expr(f, arg);
            let rep = arg.rep();
            // `None` has no value to keep.
            let local = self.reprs.local_type(rep).map(|ty| f.temp(ty));
            if let Some(local) = local {
                f.ins().local_set(local);
            }
            values.push((rep, local));
        }
        for (i, (rep, local)) in values.into_iter().enumerate() {
            if i > 0 {
                f.ins()
                    .i32_const(' ' as i32)
                    .call(self.host.write_char)
                    .call(self.host.end_write);
            }
            if let Some(local) = local {
                self.reprs.load(f, rep, local);
                f.free(local);
            }
            // The write of an object calls the `str` of its class, and the
            // write of a list the `repr` of the list.
            if matches!(
                rep,
                Rep::Uniform
                    | Rep::Object(_)
                    | Rep::ObjectOrNone(_)
                    | Rep::List(_)
                    | Rep::ListOrNone(_)
            ) {
                self.at(f, site, |e, f| e.reprs.write(&mut e.m, f, rep));
            } else {
                self.reprs.write(&mut self.m, f, rep);
            }
        }
        f.ins()
            .i32_const('\n' as i32)
            .call(self.host.write_char)
            .call(self.host.end_write);
        self.at(f, site, |e, f| {
            f.ins().global_get(e.host.error);
            f.if_(BlockType::Empty);
            f.ins()
                .global_get(e.host.error)
                .i32_const(0)
                .global_set(e.host.error);
            e.reprs.raise_os_error(f);
            f.end();
        });
    }
}

fn binding(place: Place) -> Binding {
    match place {
        Place::Global(var) => Binding::Global(var),
        Place::Local(var) => Binding::Local(var),
    }
}

fn count(n: u32) -> i32 {
    i32::try_from(n).expect("fewer than 2^31 names")
}

fn site_index(site: Site) -> i32 {
    i32::try_from(site.index()).expect("fewer than 2^31 sites")
}
