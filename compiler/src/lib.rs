//! The compiler from the Python of spython to a WasmGC module.

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Module, TypeSection,
};
use wasmparser::{BinaryReaderError, Validator, WasmFeatures};

/// The module of a program that does nothing.
///
/// It is a WASI command, so `wasmtime run` calls its `_start`.
pub fn compile() -> Result<Vec<u8>, BinaryReaderError> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("_start", ExportKind::Func, 0);
    let mut code = CodeSection::new();
    let mut start = Function::new([]);
    start.instructions().end();
    code.function(&start);

    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&exports)
        .section(&code);
    let bytes = module.finish();
    validate(&bytes)?;
    Ok(bytes)
}

/// Returns `Ok` if `bytes` is a valid module for the features the compiler
/// uses, and the reason it is not otherwise. An invalid module is a bug in
/// the compiler.
fn validate(bytes: &[u8]) -> Result<(), BinaryReaderError> {
    let features = WasmFeatures::default()
        | WasmFeatures::GC
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS;
    Validator::new_with_features(features).validate_all(bytes)?;
    Ok(())
}
