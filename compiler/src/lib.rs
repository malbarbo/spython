//! The compiler from the Python of spython to a WasmGC module.

mod emit;
mod ir;
mod lower;
mod names;
mod repr;
mod traceback;

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

/// The module of the program in `file`, which the checker has accepted.
/// A traceback names the file by `path`.
///
/// The module is a WASI command, and its `_start` runs the program.
pub fn compile(db: &dyn Db, file: File, path: &str) -> Result<Vec<u8>, Error> {
    let program = lower::lower(db, file, path)?;
    let bytes = emit::emit(&program);
    validate(&bytes).map_err(Error::Invalid)?;
    Ok(bytes)
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
