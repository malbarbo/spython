//! The `hash` of a value, which the tables of `dict` and `set` in the
//! prelude ask for.

use ruff_text_size::Ranged;

use super::{Error, Lowering, unsupported};
use crate::ir::{Expr, Rep};

impl Lowering<'_, '_> {
    /// The `hash` of `value` at `node`, as CPython gives it.
    pub(super) fn hash_value(&self, node: &impl Ranged, value: Expr) -> Result<Expr, Error> {
        match value.rep() {
            Rep::Int | Rep::Bool | Rep::Float | Rep::None => Ok(Expr::Hash(Box::new(value))),
            _ => Err(unsupported(node, "the hash of this type")),
        }
    }
}
