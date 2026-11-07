//! From the IR to the bytes of the module.

pub(crate) mod host;
pub(crate) mod module;

use wasm_encoder::BlockType;

use crate::ir::{CmpOp, Expr, Program, Rep, Stmt};
use crate::repr::Reprs;
use host::Host;
use module::{FuncState, ModuleState};

pub(crate) fn emit(program: &Program) -> Vec<u8> {
    let mut m = ModuleState::new();
    let host = Host::new(&mut m);
    let reprs = Reprs::new(&mut m, &host);
    let mut e = Emitter { m, host, reprs };
    let start_ty = e.m.func_type(&[], &[]);
    let start = e.m.reserve(start_ty);
    let mut f = FuncState::new(0);
    for stmt in &program.body {
        e.stmt(&mut f, stmt);
    }
    let init = std::mem::take(&mut e.m.init);
    e.m.define(start, f.finish_with_prefix(&init));
    e.m.export_func("_start", start);
    e.m.finish()
}

struct Emitter {
    m: ModuleState,
    host: Host,
    reprs: Reprs,
}

impl Emitter {
    fn stmt(&mut self, f: &mut FuncState, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => {
                self.expr(f, expr);
                if expr.rep() != Rep::None {
                    f.ins().drop();
                }
            }
        }
    }

    /// Pushes the value of `expr`, and nothing when its rep is `None`.
    fn expr(&mut self, f: &mut FuncState, expr: &Expr) {
        match expr {
            Expr::Str(s) => self.reprs.str_literal(&mut self.m, f, s),
            Expr::Int(value) => self.reprs.int_literal(f, *value),
            Expr::Bool(value) => self.reprs.bool_literal(f, *value),
            Expr::BoolAsInt(operand) => {
                self.expr(f, operand);
                self.reprs.bool_as_int(f);
            }
            Expr::Arith(op, left, right) => {
                self.expr(f, left);
                self.expr(f, right);
                self.reprs.int_arith(f, *op);
            }
            Expr::Compare(first, rest) => self.compare(f, first, rest),
            Expr::Print(args) => self.print(f, args),
        }
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
