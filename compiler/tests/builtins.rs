//! Checks what the linker assumes about `builtins.wasm`, the crate in
//! `compiler/builtins/` that `build.rs` compiles.

use std::collections::HashMap;

use wasmparser::{Operator, Parser, Payload};

const WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/builtins.wasm"));

/// The size of the stack, `-zstack-size` in `builtins/.cargo/config.toml`.
const STACK_SIZE: u32 = 65536;

#[derive(Default)]
struct Module {
    imports: usize,
    tables: usize,
    elements: usize,
    exports: Vec<String>,
    /// For each defined function, the bytes it takes from the stack and the
    /// functions it calls.
    funcs: Vec<(u32, Vec<u32>)>,
    stack_pointer: Option<u32>,
    forbidden: Vec<String>,
}

fn module() -> Module {
    let mut m = Module::default();
    for payload in Parser::new(0).parse_all(WASM) {
        match payload.expect("a valid module") {
            Payload::ImportSection(s) => m.imports = s.count() as usize,
            Payload::TableSection(s) => m.tables = s.count() as usize,
            Payload::ElementSection(s) => m.elements = s.count() as usize,
            Payload::ExportSection(s) => {
                for export in s {
                    let export = export.expect("an export");
                    m.exports.push(export.name.to_string());
                }
            }
            Payload::GlobalSection(s) => {
                for (i, global) in s.into_iter().enumerate() {
                    if global.expect("a global").ty.mutable {
                        assert_eq!(m.stack_pointer, None, "one mutable global");
                        m.stack_pointer = Some(i as u32);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let f = func(&body, &mut m);
                m.funcs.push(f);
            }
            _ => {}
        }
    }
    m
}

/// Returns the frame size and the callees of `body`.
///
/// rustc takes a frame with `global.get $sp`, `i32.const n`, `i32.sub`.
fn func(body: &wasmparser::FunctionBody, m: &mut Module) -> (u32, Vec<u32>) {
    let sp = m
        .stack_pointer
        .expect("the stack pointer comes before the code");
    let mut frame = 0;
    let mut calls = Vec::new();
    let mut last = [None, None];
    let reader = body.get_operators_reader().expect("a body");
    for op in reader {
        let op = op.expect("an operator");
        match &op {
            Operator::Call { function_index } => calls.push(*function_index),
            Operator::I32Sub => {
                if let [
                    Some(Operator::GlobalGet { global_index }),
                    Some(Operator::I32Const { value }),
                ] = &last
                    && *global_index == sp
                {
                    frame = frame.max(*value as u32);
                }
            }
            Operator::Unreachable
            | Operator::MemoryGrow { .. }
            | Operator::MemorySize { .. }
            | Operator::CallIndirect { .. } => m.forbidden.push(format!("{op:?}")),
            _ => {}
        }
        last = [last[1].take(), Some(op)];
    }
    (frame, calls)
}

#[test]
fn has_no_imports_tables_or_panics() {
    let m = module();
    assert_eq!(m.imports, 0);
    assert_eq!(m.tables, 0);
    assert_eq!(m.elements, 0);
    assert!(!m.exports.iter().any(|e| e == "__wasm_call_ctors"));
    assert!(m.exports.iter().any(|e| e == "__heap_base"));
    assert_eq!(m.forbidden, Vec::<String>::new());
}

/// The deepest chain of calls fits in the stack. This holds while no builtin
/// recurses, which the walk checks. With no imports, the index of a function
/// is its position in the code section.
#[test]
fn deepest_call_fits_in_the_stack() {
    let m = module();
    assert_eq!(m.imports, 0);
    let mut depth = HashMap::new();
    let deepest = (0..m.funcs.len() as u32)
        .map(|f| stack(&m, f, &mut depth, &mut Vec::new()))
        .max()
        .unwrap_or(0);
    assert!(deepest < STACK_SIZE, "{deepest} bytes");
}

fn stack(m: &Module, f: u32, depth: &mut HashMap<u32, u32>, path: &mut Vec<u32>) -> u32 {
    if let Some(&d) = depth.get(&f) {
        return d;
    }
    assert!(!path.contains(&f), "function {f} recurses");
    path.push(f);
    let (frame, calls) = m.funcs.get(f as usize).expect("a defined function");
    let d = frame
        + calls
            .iter()
            .map(|&c| stack(m, c, depth, path))
            .max()
            .unwrap_or(0);
    path.pop();
    depth.insert(f, d);
    d
}
