use wasmtime::*;

/// Compile Python source and run `_start`. Panics if any assert fails (WASM trap).
pub fn run(source: &str) {
    let wasm = compiler::compile(source).expect("compilation failed");
    let _ = std::fs::write("failed.wasm", &wasm);
    let mut config = Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    let engine = Engine::new(&config).expect("failed to create engine");
    let module = Module::new(&engine, &wasm).expect("invalid wasm");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiation failed");
    let start = instance
        .get_func(&mut store, "_start")
        .expect("_start not found");
    let mut results = [Val::I32(0)];
    start
        .call(&mut store, &[], &mut results)
        .expect("assert failed");
}

/// Compile and run, expecting a trap (assert failure).
pub fn run_expect_trap(source: &str) {
    let wasm = compiler::compile(source).expect("compilation failed");
    let mut config = Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    let engine = Engine::new(&config).expect("failed to create engine");
    let module = Module::new(&engine, &wasm).expect("invalid wasm");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiation failed");
    let start = instance
        .get_func(&mut store, "_start")
        .expect("_start not found");
    assert!(
        start.call(&mut store, &[], &mut []).is_err(),
        "expected trap"
    );
}
