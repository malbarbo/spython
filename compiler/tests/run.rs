use std::path::Path;
use std::process::{Command, Output};

/// Runs `module` in the wasmtime on the `PATH`, or in the one that
/// `WASMTIME` names. `name` names the file of the module, and has to differ
/// between tests, which run at the same time.
fn run(name: &str, module: &[u8]) -> Output {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.wasm"));
    std::fs::write(&path, module).expect("the module written to disk");
    let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
    Command::new(wasmtime)
        .args(["run", "-W", "gc=y,function-references=y,exceptions=y"])
        .args(["--invoke", "_start"])
        .arg(&path)
        .output()
        .expect("wasmtime to start")
}

#[test]
fn an_empty_program_prints_nothing() {
    let module = compiler::compile().expect("a valid module");
    let output = run("an_empty_program_prints_nothing", &module);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
