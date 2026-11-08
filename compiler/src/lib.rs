//! The compiler from the Python of spython to a WasmGC module.

mod cpython;
mod emit;
mod ir;
mod lower;
mod repr;
mod trap;

use ruff_db::files::File;
use ruff_text_size::TextRange;
use ty_python_semantic::Db;
use wasmparser::{BinaryReaderError, Validator, WasmFeatures};

/// Why a program did not compile.
#[derive(Debug)]
pub enum Error {
    /// The program uses something the compiler does not cover yet.
    Unsupported { range: TextRange, what: String },
    /// The compiler made an invalid module, which is a bug in the compiler.
    Invalid(BinaryReaderError),
}

/// A compiled program.
pub struct Module {
    /// The module, a WASI command whose `_start` runs the program.
    pub bytes: Vec<u8>,
    map: emit::module::CodeMap,
    sites: Vec<ir::SiteFrame>,
}

/// The module of the program in `file`, which the checker has accepted.
/// A traceback names the file by `path`.
pub fn compile(db: &dyn Db, file: File, path: &str) -> Result<Module, Error> {
    let program = lower::lower(db, file, path)?;
    let (bytes, map) = emit::emit(&program);
    validate(&bytes).map_err(Error::Invalid)?;
    Ok(Module {
        bytes,
        map,
        sites: program.sites,
    })
}

/// Returns `Ok` if `bytes` is a valid module for the features the compiler
/// uses, and the reason it is not otherwise.
fn validate(bytes: &[u8]) -> Result<(), BinaryReaderError> {
    let features = WasmFeatures::default()
        | WasmFeatures::GC
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS;
    Validator::new_with_features(features).validate_all(bytes)?;
    Ok(())
}
