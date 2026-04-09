use std::path::{Path, PathBuf};

use compiler::compile::{compile_file_to_wasm, compile_source_to_wasm};
use wasmtime::{Config, Engine, Instance, Module, Store};

const SUPPORTED_GENERATED_FIXTURES: &[&str] = &[
    "compiler/tests/generated/02-conceitos-basicos/exemplos/dobro.py",
    "compiler/tests/generated/02-conceitos-basicos/exemplos/hipotenusa.py",
    "compiler/tests/generated/02-conceitos-basicos/solucoes/censura.py",
    "compiler/tests/generated/02-conceitos-basicos/solucoes/unidade_dezena_centena.py",
    "compiler/tests/generated/02-conceitos-basicos/solucoes/zera_dezena_unidade.py",
    "compiler/tests/generated/03-projeto-de-programas/exemplos/custo_viagem.py",
    "compiler/tests/generated/03-projeto-de-programas/exemplos/massa_tubo_ferro.py",
    "compiler/tests/generated/03-projeto-de-programas/exemplos/numero_azulejos.py",
    "compiler/tests/generated/03-projeto-de-programas/solucoes/isento_tarifa.py",
    "compiler/tests/generated/03-projeto-de-programas/solucoes/numero_digitos.py",
    "compiler/tests/generated/04-selecao/exemplos/ajusta_numero.py",
    "compiler/tests/generated/04-selecao/exemplos/maximo.py",
    "compiler/tests/generated/04-selecao/solucoes/sinal.py",
];

#[test]
fn generated_supported_fixtures_compile_and_pass_in_wasmtime() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should live inside workspace")
        .to_path_buf();

    let engine = test_engine();

    for fixture in SUPPORTED_GENERATED_FIXTURES {
        let path = root.join(fixture);
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

fn test_engine() -> Engine {
    let mut config = Config::new();
    config.wasm_gc(true);
    config.wasm_function_references(true);
    Engine::new(&config).expect("create wasmtime engine")
}

fn run_fixture(engine: &Engine, path: &Path) {
    let output = compile_file_to_wasm(path)
        .unwrap_or_else(|err| panic!("compile failed for {}: {err}", path.display()));
    run_wasm(engine, &output.wasm, &path.display().to_string());
}

fn run_source(engine: &Engine, name: &str, source: &str) {
    let output = compile_source_to_wasm(source)
        .unwrap_or_else(|err| panic!("compile failed for {name}: {err}"));
    run_wasm(engine, &output.wasm, name);
}

fn run_wasm(engine: &Engine, wasm: &[u8], label: &str) {
    let module = Module::new(engine, wasm)
        .unwrap_or_else(|err| panic!("module failed for {label}: {err}"));
    let mut store = Store::new(engine, ());
    let instance =
        Instance::new(&mut store, &module, &[]).unwrap_or_else(|err| panic!("instantiation failed for {label}: {err}"));
    let run = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap_or_else(|err| panic!("missing `run` export for {label}: {err}"));
    let code = run
        .call(&mut store, ())
        .unwrap_or_else(|err| panic!("execution failed for {label}: {err}"));

    assert_eq!(code, 0, "assertion failed in {label}");
}
