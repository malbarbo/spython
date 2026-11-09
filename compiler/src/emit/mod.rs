//! From the IR to the bytes of the module.

pub(crate) mod module;
mod repr;
mod runtime;

use wasm_encoder::{BlockType, ConstExpr, ValType};

use crate::cpython::RECURSION_ERROR;
use crate::ir::{
    Binding, Check, CmpOp, Expr, Func, FuncId, ImportId, LogicOp, MinMaxOp, NameError, Place,
    Program, Rep, Site, Stmt, Undefined, VarId,
};
use module::{CodeMap, FuncIdx, FuncState, GlobalIdx, Label, LocalIdx, ModuleState};
use repr::Reprs;
use runtime::host::Host;

/// The bytes of the module, with the place of the code of each site.
pub(crate) fn emit(program: &Program) -> (Vec<u8>, CodeMap) {
    let mut m = ModuleState::new();
    let host = Host::new(&mut m);
    let reprs = Reprs::new(&mut m, &host);
    let vars = program
        .vars
        .iter()
        .map(|var| Slots {
            value: reprs.global(&mut m, var.rep),
            defined: var
                .flagged
                .then(|| m.global(ValType::I32, &ConstExpr::i32_const(0))),
        })
        .collect();
    let funcs = program
        .funcs
        .iter()
        .map(|func| {
            let params: Vec<ValType> = func
                .vars
                .iter()
                .take(func.params as usize)
                .map(|var| reprs.local_type(var.rep).expect("a parameter has a value"))
                .collect();
            let results: Vec<ValType> = reprs.value_type(func.ret).into_iter().collect();
            let ty = m.func_type(&params, &results);
            FuncSlots {
                index: m.reserve(ty),
                defined: m.global(ValType::I32, &ConstExpr::i32_const(0)),
            }
        })
        .collect();
    // The flag of each name that an import binds, as the flag of a `def`.
    let imports = program
        .imports
        .iter()
        .map(|_| m.global(ValType::I32, &ConstExpr::i32_const(0)))
        .collect();
    let ordered = program.vars.iter().any(|var| var.ordered)
        || program.funcs.iter().any(|func| func.ordered)
        || program.imports.iter().any(|import| import.ordered);
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
        order,
        code: Code {
            func: None,
            site,
            loops: Vec::new(),
            vars: Vec::new(),
        },
    };
    let start_ty = e.m.func_type(&[], &[]);
    let start = e.m.reserve(start_ty);
    f.ins().call(e.host.start);
    e.reprs.begin(&mut f);
    for stmt in &program.body {
        e.stmt(&mut f, stmt);
    }
    e.reprs.end_module(&mut f, site);
    for (i, func) in program.funcs.iter().enumerate() {
        e.func(FuncId::new(i), func);
    }
    e.reprs.finish(&mut e.m, &program.sites);
    let init = std::mem::take(&mut e.m.init);
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
    /// The number of ordered globals with a value, which a global gets as
    /// its flag at its first binding, when the program has an ordered
    /// global.
    order: Option<GlobalIdx>,
    code: Code,
}

/// The state of the code that the emitter is in, the module or a function.
struct Code {
    /// The function, and `None` in the module.
    func: Option<FuncId>,
    /// The local that holds the site of the instruction that may raise.
    site: LocalIdx,
    /// The target of `break` and of `continue`, for each loop
    /// that encloses the current statement.
    loops: Vec<Loop>,
    /// The locals of each variable of the function.
    vars: Vec<Slots<LocalIdx>>,
}

struct Loop {
    break_: Label,
    continue_: Label,
}

/// The globals of a variable of the module, or the locals of a variable of
/// a function.
struct Slots<I> {
    value: I,
    /// Not 0 once the variable has a value, for a flagged variable. An
    /// ordered variable holds the place of its first binding among the
    /// ordered globals, from 1.
    defined: Option<I>,
}

struct FuncSlots {
    index: FuncIdx,
    /// The global that is not 0 once the `def` has run, as the flag of a
    /// variable.
    defined: GlobalIdx,
}

impl Emitter<'_> {
    fn func(&mut self, id: FuncId, func: &Func) {
        let mut f = FuncState::new(func.params);
        let site = f.local(ValType::I32);
        let mut vars = Vec::new();
        for (i, var) in func.vars.iter().enumerate() {
            let value = match u32::try_from(i) {
                Ok(param) if param < func.params => f.param(param),
                _ => f.local(
                    self.reprs
                        .local_type(var.rep)
                        .expect("a variable has a value"),
                ),
            };
            let defined = var.flagged.then(|| f.local(ValType::I32));
            vars.push(Slots { value, defined });
        }
        // A parameter has a value from the start.
        for slots in vars.iter().take(func.params as usize) {
            if let Some(defined) = slots.defined {
                f.ins().i32_const(1).local_set(defined);
            }
        }
        self.code = Code {
            func: Some(id),
            site,
            loops: Vec::new(),
            vars,
        };
        self.reprs.enter(&mut f);
        self.reprs.begin(&mut f);
        self.body(&mut f, &func.body);
        if func.ret == Rep::None {
            self.stmt(&mut f, &Stmt::Return(None));
        }
        self.reprs.end_function(&mut f, site);
        let index = self.func_slots(id).index;
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
                let (first, rest) = targets.split_first().expect("an assignment has a target");
                self.store(f, *first);
                for place in rest {
                    match *first {
                        Place::Global(var) => f.ins().global_get(self.global_slots(var).value),
                        Place::Local(var) => f.ins().local_get(self.local_slots(var).value),
                    };
                    self.store(f, *place);
                }
            }
            Stmt::Def(func) => {
                let defined = self.func_slots(*func).defined;
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
            Stmt::Return(value) => {
                if let Some(value) = value {
                    self.expr(f, value);
                }
                self.reprs.leave(f);
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
            Stmt::Break => {
                let target = self.code.loops.last().expect("a break in a loop").break_;
                f.br(target);
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
                let mut ins = f.ins();
                ins.local_set(slots.value);
                if let Some(defined) = slots.defined {
                    ins.i32_const(1).local_set(defined);
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
            Place::Local(var) => self.reprs.load(f, rep, self.local_slots(var).value),
        }
    }

    /// Pops the flag of a name, and raises the error of `check` when it is
    /// 0.
    fn raise_if_undefined(&mut self, f: &mut FuncState, check: &Check) {
        f.ins().i32_eqz();
        f.if_(BlockType::Empty);
        self.at(f, check.site, |e, f| match &check.error {
            Undefined::Local(message) => e.reprs.raise(&mut e.m, f, message),
            Undefined::Global(error) => {
                e.name_error_message(f, error);
                e.reprs.raise_message(f);
            }
        });
        f.end();
    }

    /// Emits the code of `emit` as code that runs at `site`. An exception
    /// gets the frame of the site, and so does a trap, which wasmtime gives
    /// when the program runs out of memory.
    fn at(&mut self, f: &mut FuncState, site: Site, emit: impl FnOnce(&mut Self, &mut FuncState)) {
        f.ins()
            .i32_const(site_index(site))
            .local_set(self.code.site);
        let start = f.position();
        emit(self, f);
        f.mark(start, site);
    }

    /// Pushes the message of `error`, with the suggestion of CPython for
    /// what the frame has now.
    fn name_error_message(&mut self, f: &mut FuncState, error: &NameError) {
        if error.groups.is_empty() && error.self_attr.is_none() {
            self.reprs.str_literal(&mut self.m, f, &error.message);
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
                self.bound(f, Binding::Local(*local));
                f.ins().i32_add();
            }
            f.ins().i32_const(count(limit.max)).i32_ge_u();
            f.br_if(suggested);
        }
        for group in &error.groups {
            for (local, candidate) in &group.locals {
                self.suggest_if_bound(f, Binding::Local(*local), candidate, found, suggested);
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
        self.reprs.str_literal(&mut self.m, f, &error.message);
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

    /// Pushes an `i32` that is not 0 when `binding` has a value. The flag of
    /// a local is 1.
    fn bound(&self, f: &mut FuncState, binding: Binding) {
        match binding {
            Binding::Local(var) => match self.local_slots(var).defined {
                Some(defined) => f.ins().local_get(defined),
                // A parameter has a value from the start.
                None => f.ins().i32_const(1),
            },
            Binding::Global(var) => f
                .ins()
                .global_get(self.global_slots(var).defined.expect("a flagged global")),
            Binding::Func(func) => f.ins().global_get(self.func_slots(func).defined),
            Binding::Import(id) => f.ins().global_get(self.import_slot(id)),
        };
    }

    /// Pops an `i32`, and raises an exception with `message` when it is 0.
    fn raise_if_zero(&mut self, f: &mut FuncState, message: &str) {
        f.ins().i32_eqz();
        f.if_(BlockType::Empty);
        self.reprs.raise(&mut self.m, f, message);
        f.end();
    }

    fn call(
        &mut self,
        f: &mut FuncState,
        func: FuncId,
        args: &[Expr],
        site: Site,
        check: Option<&Check>,
    ) {
        let index = self.func_slots(func).index;
        if let Some(check) = check {
            self.bound(f, Binding::Func(func));
            self.raise_if_undefined(f, check);
        }
        for arg in args {
            self.expr(f, arg);
        }
        self.at(f, site, |e, f| {
            e.reprs.call_fits(f);
            e.raise_if_zero(f, RECURSION_ERROR);
            f.ins().call(index);
        });
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

    fn program_func(&self, func: FuncId) -> &Func {
        self.program
            .funcs
            .get(func.index())
            .expect("a function of the program")
    }

    /// Pushes the value of `expr`, and nothing when its rep is `None`.
    fn expr(&mut self, f: &mut FuncState, expr: &Expr) {
        match expr {
            Expr::Str(s) => self.reprs.str_literal(&mut self.m, f, s),
            Expr::Int(value) => self.reprs.int_literal(f, *value),
            Expr::Float(value) => self.reprs.float_literal(f, *value),
            Expr::Bool(value) => self.reprs.bool_literal(f, *value),
            Expr::Load { place, rep, check } => self.load(f, *place, *rep, check.as_ref()),
            Expr::Call {
                func,
                args,
                site,
                check,
                ..
            } => self.call(f, *func, args, *site, check.as_ref()),
            Expr::BoolAsInt(operand) => {
                self.expr(f, operand);
                self.reprs.bool_as_int(f);
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
            Expr::Index { value, index, site } => {
                self.expr(f, value);
                self.expr(f, index);
                self.at(f, *site, |e, f| e.reprs.str_index(&mut e.m, f));
            }
            Expr::Len { operand, site } => {
                self.expr(f, operand);
                self.at(f, *site, |e, f| e.reprs.str_len(f));
            }
            Expr::Truth(operand) => {
                self.expr(f, operand);
                self.reprs.truth(f, operand.rep());
            }
            Expr::Not(operand) => {
                self.expr(f, operand);
                self.reprs.bool_not(f);
            }
            Expr::Logic { op, left, right } => self.logic(f, *op, left, right),
            Expr::Compare(first, rest) => self.compare(f, first, rest),
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
            self.reprs.compare(f, rep, cmp);
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
    fn logic(&mut self, f: &mut FuncState, op: LogicOp, left: &Expr, right: &Expr) {
        let rep = left.rep();
        let left_local = f.temp(self.reprs.local_type(rep).expect("an operand has a value"));
        self.expr(f, left);
        f.ins().local_set(left_local);
        self.reprs.load(f, rep, left_local);
        self.reprs.truth(f, rep);
        f.if_(BlockType::Result(
            self.reprs.value_type(rep).expect("an operand has a value"),
        ));
        match op {
            LogicOp::And => {
                self.expr(f, right);
                f.else_();
                self.reprs.load(f, rep, left_local);
            }
            LogicOp::Or => {
                self.reprs.load(f, rep, left_local);
                f.else_();
                self.expr(f, right);
            }
        }
        f.end();
        f.free(left_local);
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
                self.reprs.compare(f, rep, *op);
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
        self.reprs.compare(f, rep, *last_op);
        f.end();
    }

    /// Python evaluates every argument before it writes the first one.
    fn print(&mut self, f: &mut FuncState, args: &[Expr], site: Site) {
        let mut values = Vec::new();
        for arg in args {
            self.expr(f, arg);
            let rep = arg.rep();
            let local = f.temp(self.reprs.local_type(rep).expect("an argument has a value"));
            f.ins().local_set(local);
            values.push((rep, local));
        }
        for (i, (rep, local)) in values.into_iter().enumerate() {
            if i > 0 {
                f.ins()
                    .i32_const(' ' as i32)
                    .call(self.host.write_char)
                    .call(self.host.end_write);
            }
            self.reprs.load(f, rep, local);
            self.reprs.write(&mut self.m, f, rep);
            f.free(local);
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
