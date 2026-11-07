//! The program after lowering. Every Python decision is already made here,
//! and the emitter only translates.

pub(crate) struct Program {
    pub body: Vec<Stmt>,
}

pub(crate) enum Stmt {
    /// An expression whose value the program drops.
    Expr(Expr),
}

pub(crate) enum Expr {
    Str(String),
    /// `print` with the default `sep` and `end`, whose arguments are all `str`.
    Print(Vec<Expr>),
}

/// The shape of a value, which knows nothing of wasm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rep {
    Str,
    None,
}

impl Expr {
    pub fn rep(&self) -> Rep {
        match self {
            Expr::Str(_) => Rep::Str,
            Expr::Print(_) => Rep::None,
        }
    }
}
