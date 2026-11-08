//! The rules of CPython 3.13 that the compiler computes in Rust, with the
//! texts that CPython writes. Nothing here knows the IR or wasm.

pub(crate) mod names;
pub(crate) mod suggest;
pub(crate) mod traceback;
