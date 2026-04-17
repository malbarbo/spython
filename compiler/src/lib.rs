//! Python-to-WASM-GC compiler for spython.
//!
//! Compiles typed Python (with mandatory annotations) to WASM with GC support.
//! Starts with Level 0: functions, scalars (int, float, bool), arithmetic.

mod codegen;
mod types;

/// Compile Python source code to a WASM binary.
///
/// Returns the WASM binary as bytes, or an error message.
pub fn compile(source: &str) -> Result<Vec<u8>, String> {
    let parsed = ruff_python_parser::parse_module(source)
        .map_err(|e| format!("Parse error: {e}"))?;
    codegen::compile_module(parsed.into_suite())
}

/// Test helpers — compile and run WASM via wasmtime.
#[cfg(test)]
pub mod test {
    /// Compile Python source and run `_start`. Panics if any assert fails (WASM trap).
    pub fn run(source: &str) {
        let wasm = crate::compile(source).expect("compilation failed");
        let mut config = wasmtime::Config::new();
        config.wasm_gc(true);
        config.wasm_function_references(true);
        let engine = wasmtime::Engine::new(&config).expect("failed to create engine");
        let module = wasmtime::Module::new(&engine, &wasm).expect("invalid wasm");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance =
            wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiation failed");
        let start = instance
            .get_func(&mut store, "_start")
            .expect("_start not found");
        start
            .call(&mut store, &[], &mut [])
            .expect("assert failed");
    }

    /// Compile and run, expecting a trap (assert failure).
    pub fn run_expect_trap(source: &str) {
        let wasm = crate::compile(source).expect("compilation failed");
        let mut config = wasmtime::Config::new();
        config.wasm_gc(true);
        config.wasm_function_references(true);
        let engine = wasmtime::Engine::new(&config).expect("failed to create engine");
        let module = wasmtime::Module::new(&engine, &wasm).expect("invalid wasm");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance =
            wasmtime::Instance::new(&mut store, &module, &[]).expect("instantiation failed");
        let start = instance
            .get_func(&mut store, "_start")
            .expect("_start not found");
        assert!(
            start.call(&mut store, &[], &mut []).is_err(),
            "expected trap"
        );
    }

}
