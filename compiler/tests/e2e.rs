use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use compiler::compile::{compile_file_to_wasm, compile_source_to_wasm};
use wasmtime::{Config, Engine, Instance, Module, Store};

const GENERATED_ASCII_LEVEL_0_1_DIRS: &[&str] = &[
    "compiler/tests/generated/02-conceitos-basicos",
    "compiler/tests/generated/03-projeto-de-programas",
    "compiler/tests/generated/04-selecao",
];

const GENERATED_ASCII_RECURSION_DIRS: &[&str] = &["compiler/tests/generated/09-recursividade"];

#[test]
fn generated_supported_fixtures_compile_and_pass_in_wasmtime() {
    let root = workspace_root();
    let engine = test_engine();
    let fixtures = discover_ascii_generated_fixtures(&root, GENERATED_ASCII_LEVEL_0_1_DIRS)
        .unwrap_or_else(|err| panic!("fixture discovery failed: {err}"));

    for path in fixtures {
        run_fixture(&engine, &path);
    }
}

#[test]
fn python_int_divmod_semantics_hold_for_negative_values() {
    let engine = test_engine();
    run_source(
        &engine,
        "python_int_divmod_semantics_hold_for_negative_values",
        r#"
def dezena(n: int) -> int:
    return n // 10 % 10

def arredonda_centena(n: int) -> int:
    return n // 100 * 100

assert dezena(152) == 5
assert dezena(-152) == 4
assert arredonda_centena(5251) == 5200
assert arredonda_centena(-152) == -200
"#,
    );
}

#[test]
fn non_ascii_string_literals_are_rejected() {
    let error = compile_source_to_wasm(
        r#"
def saudacao() -> str:
    return "olá"
"#,
    )
    .err()
    .expect("non-ASCII literal should fail");

    assert!(
        error.to_string().contains("non-ASCII string literal"),
        "unexpected error: {error}"
    );
}

#[test]
#[ignore = "exploratory coverage expansion for ASCII level 0/1 fixtures"]
fn generated_ascii_level_0_1_fixtures_compile_and_pass_in_wasmtime() {
    run_fixture_batch(
        &test_engine(),
        &discover_ascii_generated_fixtures(&workspace_root(), GENERATED_ASCII_LEVEL_0_1_DIRS)
            .unwrap_or_else(|err| panic!("fixture discovery failed: {err}")),
    );
}

#[test]
#[ignore = "exploratory coverage expansion for ASCII recursive fixtures"]
fn generated_ascii_recursion_fixtures_compile_and_pass_in_wasmtime() {
    run_fixture_batch(
        &test_engine(),
        &discover_ascii_generated_fixtures(&workspace_root(), GENERATED_ASCII_RECURSION_DIRS)
            .unwrap_or_else(|err| panic!("fixture discovery failed: {err}")),
    );
}

fn test_engine() -> Engine {
    let mut config = Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    Engine::new(&config).expect("create wasmtime engine")
}

fn run_fixture(engine: &Engine, path: &Path) {
    try_run_fixture(engine, path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
}

fn run_source(engine: &Engine, name: &str, source: &str) {
    let output = compile_source_to_wasm(source)
        .unwrap_or_else(|err| panic!("compile failed for {name}: {err}"));
    run_wasm(engine, &output.wasm, name);
}

fn try_run_fixture(engine: &Engine, path: &Path) -> Result<(), String> {
    let output = compile_file_to_wasm(path).map_err(|err| format!("compile failed: {err}"))?;
    run_wasm(engine, &output.wasm, &path.display().to_string());
    Ok(())
}

fn run_fixture_batch(engine: &Engine, fixtures: &[PathBuf]) {
    let mut failures = Vec::new();

    for fixture in fixtures {
        if let Err(err) = try_run_fixture(engine, fixture) {
            failures.push(format!("{}: {err}", fixture.display()));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} fixtures failed:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live inside workspace")
        .to_path_buf()
}

fn run_wasm(engine: &Engine, wasm: &[u8], label: &str) {
    let module =
        Module::new(engine, wasm).unwrap_or_else(|err| panic!("module failed for {label}: {err}"));
    let mut store = Store::new(engine, ());
    let instance = Instance::new(&mut store, &module, &[])
        .unwrap_or_else(|err| panic!("instantiation failed for {label}: {err}"));
    let run = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap_or_else(|err| panic!("missing `run` export for {label}: {err}"));
    let code = run
        .call(&mut store, ())
        .unwrap_or_else(|err| panic!("execution failed for {label}: {err}"));

    assert_eq!(code, 0, "assertion failed in {label}");
}

fn discover_ascii_generated_fixtures(root: &Path, dirs: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut fixtures = Vec::new();
    for dir in dirs {
        let path = root.join(dir);
        collect_ascii_py_files(&path, &mut fixtures)?;
    }
    if fixtures.is_empty() {
        return Err(format!(
            "no ASCII fixtures found under {} configured directories",
            dirs.len()
        ));
    }
    fixtures.sort();
    Ok(fixtures)
}

fn collect_ascii_py_files(dir: &Path, fixtures: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("cannot read fixture directory {}: {err}", dir.display()))?;

    for entry in entries {
        let entry =
            entry.map_err(|err| format!("cannot read entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ascii_py_files(&path, fixtures)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("py") {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|err| format!("cannot read fixture file {}: {err}", path.display()))?;
        if bytes.is_ascii() {
            fixtures.push(path);
        }
    }
    Ok(())
}

#[test]
fn fixture_discovery_fails_for_missing_directory() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("spython-missing-fixtures-{suffix}"));
    let error = discover_ascii_generated_fixtures(&root, &["missing"])
        .expect_err("missing directory should fail");

    assert!(
        error.contains("cannot read fixture directory"),
        "unexpected error: {error}"
    );
}
