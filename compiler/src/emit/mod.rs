//! From the IR to the bytes of the module.

pub(crate) mod host;
pub(crate) mod module;

use wasm_encoder::{BlockType, ConstExpr, ValType};

use crate::ir::{CmpOp, Expr, LogicOp, Program, Rep, Site, Stmt, VarId};
use crate::repr::Reprs;
use host::Host;
use module::{FuncState, ModuleState};

pub(crate) fn emit(program: &Program) -> Vec<u8> {
    let mut m = ModuleState::new();
    let host = Host::new(&mut m);
    let reprs = Reprs::new(&mut m, &host);
    let vars = program
        .vars
        .iter()
        .map(|var| VarGlobals {
            value: reprs.global(&mut m, var.rep),
            defined: var
                .checked
                .then(|| m.global(ValType::I32, &ConstExpr::i32_const(0))),
        })
        .collect();
    let mut e = Emitter {
        m,
        host,
        reprs,
        program,
        vars,
        site: 0,
        depth: 0,
        loops: Vec::new(),
    };
    let start_ty = e.m.func_type(&[], &[]);
    let start = e.m.reserve(start_ty);
    let mut f = FuncState::new(0);
    let site = f.local(ValType::I32);
    e.site = site;
    e.reprs.begin_module(&mut f);
    for stmt in &program.body {
        e.stmt(&mut f, stmt);
    }
    e.reprs.end_module(&mut f, site);
    e.reprs.finish(&mut e.m, &program.sites);
    let init = std::mem::take(&mut e.m.init);
    e.m.define(start, f.finish_with_prefix(&init));
    e.m.export_func("_start", start);
    e.m.finish()
}

struct Emitter<'a> {
    m: ModuleState,
    host: Host,
    reprs: Reprs,
    program: &'a Program,
    vars: Vec<VarGlobals>,
    /// The local that holds the site of the instruction that may raise.
    site: u32,
    /// How many blocks of statements enclose the current one.
    depth: u32,
    /// The depth of the target of `break` and of `continue`, for each loop
    /// that encloses the current statement.
    loops: Vec<Loop>,
}

enum Open {
    If,
    Block,
    Loop,
}

struct Loop {
    break_: u32,
    continue_: u32,
}

/// The globals of a variable.
struct VarGlobals {
    value: u32,
    /// 1 once the variable has a value, for a variable whose read checks it.
    defined: Option<u32>,
}

impl Emitter<'_> {
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
                for var in rest {
                    f.ins().global_get(self.globals(*first).value);
                    self.store(f, *var);
                }
            }
            Stmt::If { cond, then, else_ } => {
                self.expr(f, cond);
                self.open(f, Open::If);
                self.body(f, then);
                if !else_.is_empty() {
                    f.ins().else_();
                    self.body(f, else_);
                }
                self.close(f);
            }
            Stmt::While { cond, body, else_ } => self.while_(f, cond, body, else_),
            Stmt::Break => {
                let target = self.loops.last().expect("a break in a loop").break_;
                f.ins().br(self.depth - target);
            }
            Stmt::Continue => {
                let target = self.loops.last().expect("a continue in a loop").continue_;
                f.ins().br(self.depth - target);
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
        self.open(f, Open::Block);
        let break_ = self.depth;
        self.open(f, Open::Block);
        let done = self.depth;
        self.open(f, Open::Loop);
        let continue_ = self.depth;
        self.expr(f, cond);
        f.ins().i32_eqz().br_if(self.depth - done);
        self.loops.push(Loop { break_, continue_ });
        self.body(f, body);
        self.loops.pop();
        f.ins().br(self.depth - continue_);
        self.close(f);
        self.close(f);
        self.body(f, else_);
        self.close(f);
    }

    /// Opens a block of statements, whose depth is the new `self.depth`.
    fn open(&mut self, f: &mut FuncState, block: Open) {
        let mut ins = f.ins();
        match block {
            Open::If => ins.if_(BlockType::Empty),
            Open::Block => ins.block(BlockType::Empty),
            Open::Loop => ins.loop_(BlockType::Empty),
        };
        self.depth += 1;
    }

    fn close(&mut self, f: &mut FuncState) {
        f.ins().end();
        self.depth -= 1;
    }

    /// Pops a value into `var`.
    fn store(&mut self, f: &mut FuncState, var: VarId) {
        let globals = self.globals(var);
        f.ins().global_set(globals.value);
        if let Some(defined) = globals.defined {
            f.ins().i32_const(1).global_set(defined);
        }
    }

    fn load(&mut self, f: &mut FuncState, var: VarId, rep: Rep, check: Option<Site>) {
        if let Some(site) = check {
            let defined = self
                .globals(var)
                .defined
                .expect("a checked variable has a flag");
            f.ins()
                .global_get(defined)
                .i32_eqz()
                .if_(BlockType::Empty)
                .i32_const(site_index(site))
                .local_set(self.site);
            let message = &self
                .program
                .vars
                .get(index(var))
                .expect("a variable of the program")
                .undefined;
            self.reprs.raise(&mut self.m, f, message);
            f.ins().end();
        }
        self.reprs.load_global(f, rep, self.globals(var).value);
    }

    fn globals(&self, var: VarId) -> &VarGlobals {
        self.vars
            .get(index(var))
            .expect("a variable of the program")
    }

    /// Pushes the value of `expr`, and nothing when its rep is `None`.
    fn expr(&mut self, f: &mut FuncState, expr: &Expr) {
        match expr {
            Expr::Str(s) => self.reprs.str_literal(&mut self.m, f, s),
            Expr::Int(value) => self.reprs.int_literal(f, *value),
            Expr::Bool(value) => self.reprs.bool_literal(f, *value),
            Expr::Load { var, rep, check } => self.load(f, *var, *rep, *check),
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
                f.ins().i32_const(site_index(*site)).local_set(self.site);
                self.reprs.int_arith(f, *op);
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
            Expr::Print(args) => self.print(f, args),
        }
    }

    /// The left operand goes to a local, which gives both its truth and,
    /// when the truth decides the result, the result.
    fn logic(&mut self, f: &mut FuncState, op: LogicOp, left: &Expr, right: &Expr) {
        let rep = left.rep();
        let left_local = f.local(self.reprs.local_type(rep).expect("an operand has a value"));
        self.expr(f, left);
        f.ins().local_set(left_local);
        self.reprs.load(f, rep, left_local);
        self.reprs.truth(f, rep);
        f.ins().if_(BlockType::Result(
            self.reprs.value_type(rep).expect("an operand has a value"),
        ));
        match op {
            LogicOp::And => {
                self.expr(f, right);
                f.ins().else_();
                self.reprs.load(f, rep, left_local);
            }
            LogicOp::Or => {
                self.reprs.load(f, rep, left_local);
                f.ins().else_();
                self.expr(f, right);
            }
        }
        f.ins().end();
    }

    /// A chain stops at the first comparison that is false, and the operands
    /// after it are not evaluated.
    fn compare(&mut self, f: &mut FuncState, first: &Expr, rest: &[(CmpOp, Expr)]) {
        let Some(((last_op, last), init)) = rest.split_last() else {
            unreachable!("a comparison without operators");
        };
        f.ins().block(BlockType::Result(
            self.reprs
                .local_type(Rep::Bool)
                .expect("a bool has a value"),
        ));
        self.expr(f, first);
        if !init.is_empty() {
            let right = f.local(self.reprs.local_type(Rep::Int).expect("an int has a value"));
            for (op, operand) in init {
                self.expr(f, operand);
                f.ins().local_tee(right);
                self.reprs.int_compare(f, *op);
                f.ins().i32_eqz().if_(BlockType::Empty);
                self.reprs.bool_literal(f, false);
                f.ins().br(1).end();
                self.reprs.load(f, Rep::Int, right);
            }
        }
        self.expr(f, last);
        self.reprs.int_compare(f, *last_op);
        f.ins().end();
    }

    /// Python evaluates every argument before it writes the first one.
    fn print(&mut self, f: &mut FuncState, args: &[Expr]) {
        let mut values = Vec::new();
        for arg in args {
            self.expr(f, arg);
            let rep = arg.rep();
            let local = f.local(self.reprs.local_type(rep).expect("an argument has a value"));
            f.ins().local_set(local);
            values.push((rep, local));
        }
        for (i, (rep, local)) in values.into_iter().enumerate() {
            if i > 0 {
                f.ins().i32_const(' ' as i32).call(self.host.write_char);
            }
            self.reprs.load(f, rep, local);
            self.reprs.write(&mut self.m, f, rep);
        }
        f.ins()
            .i32_const('\n' as i32)
            .call(self.host.write_char)
            .call(self.host.flush);
    }
}

fn index(var: VarId) -> usize {
    var.0 as usize
}

fn site_index(site: Site) -> i32 {
    i32::try_from(site.0).expect("fewer than 2^31 sites")
}
