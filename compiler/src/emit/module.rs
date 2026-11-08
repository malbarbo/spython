//! The state of the module and the state of one function while they are
//! emitted.

use std::collections::HashMap;
use std::ops::Range;

use wasm_encoder::{
    CodeSection, ConstExpr, DataCountSection, DataSection, EntityType, ExportKind, ExportSection,
    FieldType, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    InstructionSink, MemorySection, MemoryType, Module, StorageType, TagKind, TagSection, TagType,
    TypeSection, ValType,
};
use wasmparser::{Parser, Payload};

use crate::ir::Site;

pub(crate) struct ModuleState {
    types: TypeSection,
    type_count: u32,
    func_types: HashMap<(Vec<ValType>, Vec<ValType>), u32>,
    imports: ImportSection,
    imported_funcs: u32,
    /// The type and the body of each function the module defines, in the
    /// order of their indices. A reserved function has no body yet.
    funcs: Vec<(u32, Option<Function>)>,
    memories: MemorySection,
    tags: TagSection,
    tag_count: u32,
    globals: GlobalSection,
    global_count: u32,
    exports: ExportSection,
    data: DataSection,
    data_count: u32,
    /// The code `_start` runs before the program, which fills the globals.
    pub init: Vec<u8>,
    /// The code of each function that runs the code of a Python frame, by
    /// the index of the function among the functions the module defines.
    frames: Vec<(usize, Vec<SiteRange>)>,
}

/// The code from `start` to `end` that runs at `site`. Inside a function,
/// both count from the start of its instructions, and in [`CodeMap`] they
/// count from the start of the module.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SiteRange {
    pub start: u32,
    pub end: u32,
    pub site: Site,
}

/// Where the code of the Python frames is in the module.
pub(crate) struct CodeMap {
    /// The code of each function that runs the code of a Python frame.
    pub frames: Vec<Range<u32>>,
    /// The code of the frame of the module, the outermost frame.
    pub module: Range<u32>,
    pub sites: Vec<SiteRange>,
}

impl ModuleState {
    pub fn new() -> Self {
        ModuleState {
            types: TypeSection::new(),
            type_count: 0,
            func_types: HashMap::new(),
            imports: ImportSection::new(),
            imported_funcs: 0,
            funcs: Vec::new(),
            memories: MemorySection::new(),
            tags: TagSection::new(),
            tag_count: 0,
            globals: GlobalSection::new(),
            global_count: 0,
            exports: ExportSection::new(),
            data: DataSection::new(),
            data_count: 0,
            init: Vec::new(),
            frames: Vec::new(),
        }
    }

    pub fn func_type(&mut self, params: &[ValType], results: &[ValType]) -> u32 {
        let key = (params.to_vec(), results.to_vec());
        if let Some(&index) = self.func_types.get(&key) {
            return index;
        }
        self.types
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        let index = self.next_type();
        self.func_types.insert(key, index);
        index
    }

    pub fn array_type(&mut self, element: StorageType, mutable: bool) -> u32 {
        self.types.ty().array(&element, mutable);
        self.next_type()
    }

    /// A struct type with the fields, each with its type and whether it is
    /// mutable. A field can refer to the struct by [`Self::next_type_index`].
    pub fn struct_type(&mut self, fields: &[(StorageType, bool)]) -> u32 {
        self.types
            .ty()
            .struct_(fields.iter().map(|&(element_type, mutable)| FieldType {
                element_type,
                mutable,
            }));
        self.next_type()
    }

    /// The index the next type will have.
    pub fn next_type_index(&self) -> u32 {
        self.type_count
    }

    /// An exception tag whose payload is the parameters of `func_type`.
    pub fn tag(&mut self, func_type: u32) -> u32 {
        self.tags.tag(TagType {
            kind: TagKind::Exception,
            func_type_idx: func_type,
        });
        self.tag_count += 1;
        self.tag_count - 1
    }

    fn next_type(&mut self) -> u32 {
        self.type_count += 1;
        self.type_count - 1
    }

    /// Imports a function. The imports have to come before every function
    /// the module defines, because they take the first indices.
    pub fn import_func(&mut self, module: &str, name: &str, ty: u32) -> u32 {
        assert!(
            self.funcs.is_empty(),
            "an import after a defined function shifts its index"
        );
        self.imports.import(module, name, EntityType::Function(ty));
        self.imported_funcs += 1;
        self.imported_funcs - 1
    }

    /// The index of a new function, whose body [`Self::define`] gives later.
    pub fn reserve(&mut self, ty: u32) -> u32 {
        self.funcs.push((ty, None));
        self.imported_funcs
            + u32::try_from(self.funcs.len() - 1).expect("fewer than 2^32 functions")
    }

    pub fn define(&mut self, index: u32, body: Function) {
        let slot = self
            .funcs
            .get_mut((index - self.imported_funcs) as usize)
            .expect("a reserved function");
        assert!(slot.1.is_none(), "function {index} defined twice");
        slot.1 = Some(body);
    }

    /// Defines the function of a Python frame, which runs `prefix` before
    /// the code of `f`.
    pub fn define_frame(&mut self, index: u32, f: FuncState, prefix: &[u8]) {
        let shift = u32::try_from(prefix.len()).expect("a prefix shorter than 2^32 bytes");
        let sites = f
            .sites
            .iter()
            .map(|range| SiteRange {
                start: range.start + shift,
                end: range.end + shift,
                site: range.site,
            })
            .collect();
        self.frames
            .push(((index - self.imported_funcs) as usize, sites));
        self.define(index, f.finish_with_prefix(prefix));
    }

    pub fn add_func(&mut self, ty: u32, body: Function) -> u32 {
        let index = self.reserve(ty);
        self.define(index, body);
        index
    }

    pub fn memory(&mut self, export: &str) {
        self.memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        self.exports.export(export, ExportKind::Memory, 0);
    }

    pub fn global(&mut self, ty: ValType, init: &ConstExpr) -> u32 {
        self.globals.global(
            GlobalType {
                val_type: ty,
                mutable: true,
                shared: false,
            },
            init,
        );
        self.global_count += 1;
        self.global_count - 1
    }

    pub fn passive_data(&mut self, bytes: Vec<u8>) -> u32 {
        self.data.passive(bytes);
        self.data_count += 1;
        self.data_count - 1
    }

    pub fn export_func(&mut self, name: &str, index: u32) {
        self.exports.export(name, ExportKind::Func, index);
    }

    /// The bytes of the module, with the place of the code of the Python
    /// frames. The function `module_frame` runs the frame of the module.
    pub fn finish(self, module_frame: u32) -> (Vec<u8>, CodeMap) {
        let mut functions = FunctionSection::new();
        let mut code = CodeSection::new();
        for (index, (ty, body)) in self.funcs.iter().enumerate() {
            functions.function(*ty);
            let body = body
                .as_ref()
                .unwrap_or_else(|| panic!("function {index} reserved and never defined"));
            code.function(body);
        }
        let mut module = Module::new();
        module
            .section(&self.types)
            .section(&self.imports)
            .section(&functions)
            .section(&self.memories)
            .section(&self.tags)
            .section(&self.globals)
            .section(&self.exports)
            .section(&DataCountSection {
                count: self.data_count,
            })
            .section(&code)
            .section(&self.data);
        let bytes = module.finish();
        let map = code_map(
            &bytes,
            &self.frames,
            (module_frame - self.imported_funcs) as usize,
        );
        (bytes, map)
    }
}

/// The offsets in `bytes` of the code of `frames`, whose ranges count from
/// the start of the instructions of their function. The function `module`
/// runs the frame of the module.
fn code_map(bytes: &[u8], frames: &[(usize, Vec<SiteRange>)], module: usize) -> CodeMap {
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CodeSectionEntry(body) = payload.expect("a module the emitter made") {
            let start = body
                .get_operators_reader()
                .expect("a body the emitter made")
                .original_position();
            bodies.push((body.range(), start));
        }
    }
    let (range, _) = bodies.get(module).expect("a function the module defines");
    let mut map = CodeMap {
        frames: Vec::new(),
        module: offset(range.start)..offset(range.end),
        sites: Vec::new(),
    };
    for (func, sites) in frames {
        let (range, start) = bodies.get(*func).expect("a function the module defines");
        map.frames.push(offset(range.start)..offset(range.end));
        let start = offset(*start);
        map.sites.extend(sites.iter().map(|range| SiteRange {
            start: start + range.start,
            end: start + range.end,
            site: range.site,
        }));
    }
    map
}

fn offset(n: impl TryInto<u32>) -> u32 {
    n.try_into()
        .unwrap_or_else(|_| panic!("a module shorter than 2^32 bytes"))
}

/// The locals and the code of a function while it is emitted. A local is
/// added when the code asks for one.
pub(crate) struct FuncState {
    params: u32,
    locals: Vec<ValType>,
    /// The temporaries that no code uses now.
    free: Vec<(ValType, u32)>,
    code: Vec<u8>,
    sites: Vec<SiteRange>,
}

impl FuncState {
    pub fn new(params: u32) -> Self {
        FuncState {
            params,
            locals: Vec::new(),
            free: Vec::new(),
            code: Vec::new(),
            sites: Vec::new(),
        }
    }

    /// The offset of the next instruction from the start of the code.
    pub fn position(&self) -> u32 {
        u32::try_from(self.code.len()).expect("a function shorter than 2^32 bytes")
    }

    /// Marks the code from `start` to the next instruction as code that
    /// runs at `site`.
    pub fn mark(&mut self, start: u32, site: Site) {
        let end = self.position();
        self.sites.push(SiteRange { start, end, site });
    }

    pub fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        self.params + u32::try_from(self.locals.len() - 1).expect("fewer than 2^32 locals")
    }

    /// A local for a value that lives inside one expression, which goes
    /// back with `free`. Its value is whatever the last user left, so the
    /// code sets it before it reads it.
    pub fn temp(&mut self, ty: ValType) -> u32 {
        match self.free.iter().position(|&(free, _)| free == ty) {
            Some(i) => self.free.swap_remove(i).1,
            None => self.local(ty),
        }
    }

    pub fn free(&mut self, local: u32) {
        let ty = local
            .checked_sub(self.params)
            .and_then(|i| self.locals.get(usize::try_from(i).ok()?))
            .expect("a temporary is a local of the function");
        self.free.push((*ty, local));
    }

    pub fn ins(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.code)
    }

    /// The function, with `prefix` run before its own code. `prefix` uses
    /// no local.
    fn finish_with_prefix(self, prefix: &[u8]) -> Function {
        let mut function = Function::new_with_locals_types(self.locals);
        function.raw(prefix.iter().copied());
        function.raw(self.code);
        function.instructions().end();
        function
    }

    pub fn finish(self) -> Function {
        self.finish_with_prefix(&[])
    }
}
