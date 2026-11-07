//! How each value is laid out in wasm. No other module knows it.

mod str;

use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};
use str::StrRepr;

pub(crate) struct Reprs {
    str: StrRepr,
}

impl Reprs {
    pub fn new(m: &mut ModuleState, host: &Host) -> Self {
        Reprs {
            str: StrRepr::ucs4(m, host),
        }
    }

    /// Pushes the `str` with the text `s`.
    pub fn str_literal(&mut self, m: &mut ModuleState, f: &mut FuncState, s: &str) {
        self.str.literal(m, f, s);
    }

    /// Pops a `str` and writes it to the standard output.
    pub fn write_str(&self, f: &mut FuncState) {
        self.str.write(f);
    }
}
