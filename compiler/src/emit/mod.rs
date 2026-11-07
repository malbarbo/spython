//! From the IR to the bytes of the module.

pub(crate) mod host;
pub(crate) mod module;

use crate::ir::{Expr, Program, Rep, Stmt};
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
            Expr::Print(args) => {
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        f.ins().i32_const(' ' as i32).call(self.host.write_char);
                    }
                    self.expr(f, arg);
                    self.reprs.write_str(f);
                }
                f.ins()
                    .i32_const('\n' as i32)
                    .call(self.host.write_char)
                    .call(self.host.flush);
            }
        }
    }
}
