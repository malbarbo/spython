//! The functions of the crate in `compiler/builtins/`, which `build.rs`
//! compiles to wasm32. The module takes the functions that it calls, the
//! functions that they call, the stack pointer and the data segments that
//! those functions use.
//!
//! Linear memory starts with the stack of the builtins, then their data,
//! then the area that [`free_memory`] gives to the host. The layout is the
//! same whether the module calls a builtin or not.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::convert::Infallible;
use std::sync::LazyLock;

use wasm_encoder::reencode::{Error, Reencode, RoundtripReencoder};
use wasm_encoder::{ConstExpr, Function, ValType};
use wasmparser::{FunctionBody, KnownCustom, Linking, Operator, Parser, Payload, SymbolInfo};

use super::{FuncIdx, GlobalIdx, ModuleState, TypeIdx};

static WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/builtins.wasm"));

static LIBRARY: LazyLock<Library> = LazyLock::new(|| Library::parse(WASM));

/// The first address of linear memory that the builtins leave free.
pub(crate) fn free_memory() -> i32 {
    LIBRARY.free
}

/// What the linker reads from `builtins.wasm`.
struct Library {
    /// The parameters and the results of each type.
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    /// The type of each function.
    func_types: Vec<u32>,
    bodies: Vec<FunctionBody<'static>>,
    /// The function of each export, by its name.
    exports: BTreeMap<&'static str, u32>,
    stack_pointer: u32,
    /// The value of the stack pointer at the start, the top of the stack.
    stack_top: i32,
    /// Each active segment, with its address.
    data: Vec<(i32, &'static [u8])>,
    /// The data segments that the code of each function refers to.
    func_segments: Vec<BTreeSet<usize>>,
    free: i32,
}

impl Library {
    fn parse(wasm: &'static [u8]) -> Library {
        let mut lib = Library {
            types: Vec::new(),
            func_types: Vec::new(),
            bodies: Vec::new(),
            exports: BTreeMap::new(),
            stack_pointer: 0,
            stack_top: 0,
            data: Vec::new(),
            func_segments: Vec::new(),
            free: 0,
        };
        let mut heap_base = None;
        let mut globals = Vec::new();
        let mut code_start = 0;
        // The segment of each symbol, `None` for a symbol that is not data.
        let mut symbols: Vec<Option<usize>> = Vec::new();
        // The offset in the code section and the symbol of each relocation.
        let mut relocations = Vec::new();
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.expect("builtins.wasm is valid") {
                Payload::TypeSection(s) => {
                    for ty in s.into_iter_err_on_gc_types() {
                        let ty = ty.expect("a function type");
                        let types = |types: &[wasmparser::ValType]| {
                            types
                                .iter()
                                .map(|&ty| RoundtripReencoder.val_type(ty).expect("a value type"))
                                .collect()
                        };
                        lib.types.push((types(ty.params()), types(ty.results())));
                    }
                }
                Payload::ImportSection(s) => {
                    assert_eq!(s.count(), 0, "builtins.wasm imports nothing");
                }
                Payload::FunctionSection(s) => {
                    for ty in s {
                        lib.func_types.push(ty.expect("a function"));
                    }
                }
                Payload::GlobalSection(s) => {
                    for global in s {
                        let global = global.expect("a global");
                        globals.push((global.ty.mutable, const_i32(&global.init_expr)));
                    }
                }
                Payload::ExportSection(s) => {
                    for export in s {
                        let export = export.expect("an export");
                        match export.kind {
                            wasmparser::ExternalKind::Func => {
                                lib.exports.insert(export.name, export.index);
                            }
                            wasmparser::ExternalKind::Global if export.name == "__heap_base" => {
                                heap_base = Some(export.index);
                            }
                            _ => {}
                        }
                    }
                }
                Payload::DataSection(s) => {
                    for data in s {
                        let data = data.expect("a segment");
                        let wasmparser::DataKind::Active { offset_expr, .. } = data.kind else {
                            panic!("builtins.wasm has only active segments");
                        };
                        lib.data.push((const_i32(&offset_expr), data.data));
                    }
                }
                Payload::CodeSectionStart {
                    unchecked_range, ..
                } => code_start = unchecked_range.start,
                Payload::CodeSectionEntry(body) => lib.bodies.push(body),
                Payload::CustomSection(s) => match s.as_known() {
                    KnownCustom::Linking(linking) => {
                        for subsection in linking {
                            if let Linking::SymbolTable(table) = subsection.expect("a subsection") {
                                for symbol in table {
                                    symbols.push(match symbol.expect("a symbol") {
                                        SymbolInfo::Data {
                                            symbol: Some(data), ..
                                        } => Some(data.index as usize),
                                        _ => None,
                                    });
                                }
                            }
                        }
                    }
                    KnownCustom::Reloc(reloc) => {
                        let entries = reloc.entries().into_iter();
                        let entries = entries.map(|entry| entry.expect("a relocation"));
                        if s.name() == "reloc.CODE" {
                            relocations.extend(entries.map(|entry| (entry.offset, entry.index)));
                        } else {
                            assert_eq!(entries.count(), 0, "only the code has relocations");
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let mut mutable = globals
            .iter()
            .enumerate()
            .filter(|(_, (mutable, _))| *mutable);
        let (stack_pointer, (_, stack_top)) = mutable.next().expect("a stack pointer");
        assert!(mutable.next().is_none(), "one mutable global");
        lib.stack_pointer = u32::try_from(stack_pointer).expect("fewer than 2^32 globals");
        lib.stack_top = *stack_top;
        assert!(
            lib.data.iter().all(|&(at, _)| at >= lib.stack_top),
            "the stack comes before the data, so an overflow traps"
        );
        let heap_base = heap_base.expect("builtins.wasm exports __heap_base");
        let (_, heap_base) = globals
            .get(heap_base as usize)
            .expect("a global of builtins.wasm");
        lib.free = heap_base.checked_add(15).expect("a heap base below 2^31") & !15;

        assert!(!symbols.is_empty(), "builtins.wasm has a symbol table");
        lib.func_segments = vec![BTreeSet::new(); lib.bodies.len()];
        for (offset, symbol) in relocations {
            let segment = *symbols.get(symbol as usize).expect("a symbol");
            let Some(segment) = segment else { continue };
            let at = code_start + u64::from(offset);
            let func = lib
                .bodies
                .iter()
                .position(|body| body.range().contains(&at))
                .expect("a relocation in a function");
            lib.func_segments
                .get_mut(func)
                .expect("a function")
                .insert(segment);
        }
        lib
    }

    fn func_type(&self, func: u32) -> &(Vec<ValType>, Vec<ValType>) {
        let ty = self
            .func_types
            .get(func as usize)
            .expect("a function of builtins.wasm");
        self.types
            .get(*ty as usize)
            .expect("a type of builtins.wasm")
    }

    fn body(&self, func: u32) -> &FunctionBody<'static> {
        self.bodies
            .get(func as usize)
            .expect("a function of builtins.wasm")
    }

    /// The data segments that `funcs` refer to.
    fn segments(&self, funcs: &[u32]) -> BTreeSet<usize> {
        funcs
            .iter()
            .flat_map(|&func| {
                self.func_segments
                    .get(func as usize)
                    .expect("a function of builtins.wasm")
            })
            .copied()
            .collect()
    }
}

/// The value of a constant expression that is one `i32.const`.
fn const_i32(expr: &wasmparser::ConstExpr) -> i32 {
    match expr.get_operators_reader().read() {
        Ok(Operator::I32Const { value }) => value,
        op => panic!("a constant i32, not {op:?}"),
    }
}

/// The functions of `builtins.wasm` that the module has.
#[derive(Default)]
pub(super) struct Linked {
    /// The index in the module of each function, by its index in
    /// `builtins.wasm`.
    funcs: BTreeMap<u32, FuncIdx>,
}

impl ModuleState {
    /// The function that `builtins.wasm` exports as `name`.
    pub fn builtin(&mut self, name: &str) -> FuncIdx {
        let func = *LIBRARY
            .exports
            .get(name)
            .unwrap_or_else(|| panic!("builtins.wasm exports no {name}"));
        self.builtin_func(func)
    }

    fn builtin_func(&mut self, func: u32) -> FuncIdx {
        if let Some(&index) = self.builtins.funcs.get(&func) {
            return index;
        }
        let (params, results) = LIBRARY.func_type(func);
        let ty = self.func_type(params, results);
        let index = self.reserve(ty);
        self.builtins.funcs.insert(func, index);
        index
    }

    /// Defines the functions of `builtins.wasm` that the module asked for,
    /// with the functions they call, and adds the stack pointer and the data
    /// segments that they use. The functions they call come after every
    /// other function.
    pub(super) fn link_builtins(&mut self) {
        if self.builtins.funcs.is_empty() {
            return;
        }
        let mut queue: VecDeque<u32> = self.builtins.funcs.keys().copied().collect();
        let mut linked = Vec::new();
        while let Some(func) = queue.pop_front() {
            linked.push(func);
            let reader = LIBRARY
                .body(func)
                .get_operators_reader()
                .expect("builtins.wasm is valid");
            for op in reader {
                if let Operator::Call { function_index } = op.expect("builtins.wasm is valid")
                    && !self.builtins.funcs.contains_key(&function_index)
                {
                    self.builtin_func(function_index);
                    queue.push_back(function_index);
                }
            }
        }
        let stack_pointer = self.global(ValType::I32, &ConstExpr::i32_const(LIBRARY.stack_top));
        let segments = LIBRARY.segments(&linked);
        for func in linked {
            let index = *self.builtins.funcs.get(&func).expect("a linked function");
            let mut r = Relink {
                m: self,
                stack_pointer,
            };
            let body = r.body(LIBRARY.body(func));
            self.define(index, body);
        }
        for segment in segments {
            let (at, bytes) = *LIBRARY.data.get(segment).expect("a segment");
            self.data
                .active(0, &ConstExpr::i32_const(at), bytes.iter().copied());
            self.data_count += 1;
        }
    }
}

/// Gives the functions, the types and the stack pointer of `builtins.wasm`
/// their indices in the module.
struct Relink<'a> {
    m: &'a mut ModuleState,
    stack_pointer: GlobalIdx,
}

impl Relink<'_> {
    fn body(&mut self, body: &FunctionBody) -> Function {
        let mut function = self
            .new_function_with_parsed_locals(body)
            .expect("builtins.wasm is valid");
        let mut reader = body.get_operators_reader().expect("builtins.wasm is valid");
        while !reader.eof() {
            let ins = self
                .parse_instruction(&mut reader)
                .expect("builtins.wasm is valid");
            function.instruction(&ins);
        }
        function
    }
}

impl Reencode for Relink<'_> {
    type Error = Infallible;

    fn function_index(&mut self, func: u32) -> Result<u32, Error<Infallible>> {
        let index = self.m.builtins.funcs.get(&func).expect("a linked function");
        Ok(index.0)
    }

    fn type_index(&mut self, ty: u32) -> Result<u32, Error<Infallible>> {
        let (params, results) = LIBRARY.types.get(ty as usize).expect("a type");
        let TypeIdx(index) = self.m.func_type(params, results);
        Ok(index)
    }

    fn global_index(&mut self, global: u32) -> Result<u32, Error<Infallible>> {
        assert_eq!(
            global, LIBRARY.stack_pointer,
            "a builtin reads no other global"
        );
        Ok(self.stack_pointer.0)
    }

    fn data_index(&mut self, _: u32) -> Result<u32, Error<Infallible>> {
        panic!("builtins.wasm has no passive segment")
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use wasm_encoder::{Function, MemArg, MemArgTarget, ValType};

    use super::free_memory;
    use crate::emit::module::ModuleState;

    /// Where the module writes the `repr`, and the iovec and the count of
    /// `fd_write`.
    const OUT: i32 = 0;
    const IOVEC: i32 = 32;
    const NWRITTEN: i32 = 40;

    /// A module that writes a line with the `repr` of each of `floats`,
    /// through `f64_repr`.
    fn module(floats: &[f64]) -> Vec<u8> {
        let mut m = ModuleState::new();
        let fd_write_ty = m.func_type(&[ValType::I32; 4], &[ValType::I32]);
        let fd_write = m.import_func("wasi_snapshot_preview1", "fd_write", fd_write_ty);
        m.memory("memory", free_memory() + 64);
        let repr = m.builtin("f64_repr");
        let at = |offset| free_memory() + offset;
        let mem = |offset: i32| MemArg {
            offset: u64::try_from(offset).expect("a positive offset"),
            align: 0,
            target: MemArgTarget::Memory(0),
            ordering: None,
        };
        let len = 0;
        let mut f = Function::new([(1, ValType::I32)]);
        for &x in floats {
            f.instructions()
                .f64_const(x.into())
                .i32_const(at(OUT))
                .call(repr.0)
                .local_set(len)
                .local_get(len)
                .i32_const(i32::from(b'\n'))
                .i32_store8(mem(at(OUT)))
                .i32_const(0)
                .i32_const(at(OUT))
                .i32_store(mem(at(IOVEC)))
                .i32_const(0)
                .local_get(len)
                .i32_const(1)
                .i32_add()
                .i32_store(mem(at(IOVEC) + 4))
                .i32_const(1)
                .i32_const(at(IOVEC))
                .i32_const(1)
                .i32_const(at(NWRITTEN))
                .call(fd_write.0)
                .drop();
        }
        f.instructions().end();
        let ty = m.func_type(&[], &[]);
        let start = m.add_func(ty, f);
        m.export_func("_start", start);
        m.finish(start).0
    }

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("spython-{}-{name}.wasm", std::process::id()))
    }

    fn stdout(mut command: Command) -> String {
        let output = command.output().expect("the runner runs");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("ASCII")
    }

    #[test]
    fn a_module_calls_f64_repr() {
        let floats = [
            2.5,
            -0.0,
            1e16,
            1e-5,
            0.1 + 0.2,
            f64::NEG_INFINITY,
            f64::NAN,
            5e-324,
        ];
        let expected = "2.5\n-0.0\n1e+16\n1e-05\n0.30000000000000004\n-inf\nnan\n5e-324\n";
        let path = path("f64_repr");
        std::fs::write(&path, module(&floats)).expect("the module written to disk");

        let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
        let mut command = Command::new(wasmtime);
        command.args(["run", "--invoke", "_start"]).arg(&path);
        assert_eq!(stdout(command), expected, "wasmtime");

        let deno = std::env::var("DENO").unwrap_or_else(|_| "deno".to_owned());
        let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wasi.ts");
        let mut command = Command::new(deno);
        command
            .args(["run", "--quiet", "--allow-read"])
            .arg(runner)
            .arg(&path);
        assert_eq!(stdout(command), expected, "deno");
        std::fs::remove_file(&path).expect("the module removed");
    }
}
