//! The state of the module and the state of one function while they are
//! emitted.

use std::collections::HashMap;
use std::ops::Range;

use wasm_encoder::{
    BlockType, Catch, CodeSection, ConstExpr, DataCountSection, DataSection, EntityType,
    ExportKind, ExportSection, FieldType, Function, FunctionSection, GlobalSection, GlobalType,
    HeapType, Ieee64, ImportSection, InstructionSink, MemArg, MemorySection, MemoryType, Module,
    RefType, StorageType, TagKind, TagSection, TagType, TypeSection, ValType,
};
use wasmparser::{Parser, Payload};

use crate::ir::Site;

mod builtins;

pub(crate) use builtins::free_memory;

/// The size of a page of linear memory.
const PAGE: u64 = 65536;

/// The index of a function in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FuncIdx(u32);

/// The index of a global in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlobalIdx(u32);

/// The index of a type in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypeIdx(u32);

impl TypeIdx {
    pub fn heap_type(self) -> HeapType {
        HeapType::Concrete(self.0)
    }

    /// The type of a reference to a value of this type.
    pub fn ref_to(self, nullable: bool) -> ValType {
        ValType::Ref(RefType {
            nullable,
            heap_type: self.heap_type(),
        })
    }
}

/// The index of an exception tag in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TagIdx(u32);

/// The index of a data segment in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DataIdx(u32);

/// The index of a local, or of a parameter, in a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalIdx(u32);

pub(crate) struct ModuleState {
    types: TypeSection,
    type_count: u32,
    func_types: HashMap<(Vec<ValType>, Vec<ValType>), TypeIdx>,
    array_types: HashMap<(StorageType, bool), TypeIdx>,
    imports: ImportSection,
    imported_funcs: u32,
    /// The type and the body of each function the module defines, in the
    /// order of their indices. A reserved function has no body yet.
    funcs: Vec<(TypeIdx, Option<Function>)>,
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
    builtins: builtins::Linked,
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
            array_types: HashMap::new(),
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
            builtins: builtins::Linked::default(),
        }
    }

    pub fn func_type(&mut self, params: &[ValType], results: &[ValType]) -> TypeIdx {
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

    pub fn array_type(&mut self, element: StorageType, mutable: bool) -> TypeIdx {
        if let Some(&index) = self.array_types.get(&(element, mutable)) {
            return index;
        }
        self.types.ty().array(&element, mutable);
        let index = self.next_type();
        self.array_types.insert((element, mutable), index);
        index
    }

    /// A struct type with the fields, each with its type and whether it is
    /// mutable. A field can refer to the struct by [`Self::next_type_index`].
    pub fn struct_type(&mut self, fields: &[(StorageType, bool)]) -> TypeIdx {
        self.types
            .ty()
            .struct_(fields.iter().map(|&(element_type, mutable)| FieldType {
                element_type,
                mutable,
            }));
        self.next_type()
    }

    /// The index the next type will have.
    pub fn next_type_index(&self) -> TypeIdx {
        TypeIdx(self.type_count)
    }

    /// An exception tag whose payload is the parameters of `func_type`.
    pub fn tag(&mut self, func_type: TypeIdx) -> TagIdx {
        self.tags.tag(TagType {
            kind: TagKind::Exception,
            func_type_idx: func_type.0,
        });
        self.tag_count += 1;
        TagIdx(self.tag_count - 1)
    }

    fn next_type(&mut self) -> TypeIdx {
        self.type_count += 1;
        TypeIdx(self.type_count - 1)
    }

    /// Imports a function. The imports have to come before every function
    /// the module defines, because they take the first indices.
    pub fn import_func(&mut self, module: &str, name: &str, ty: TypeIdx) -> FuncIdx {
        assert!(
            self.funcs.is_empty(),
            "an import after a defined function shifts its index"
        );
        self.imports
            .import(module, name, EntityType::Function(ty.0));
        self.imported_funcs += 1;
        FuncIdx(self.imported_funcs - 1)
    }

    /// The index of a new function, whose body [`Self::define`] gives later.
    pub fn reserve(&mut self, ty: TypeIdx) -> FuncIdx {
        self.funcs.push((ty, None));
        FuncIdx(
            self.imported_funcs
                + u32::try_from(self.funcs.len() - 1).expect("fewer than 2^32 functions"),
        )
    }

    pub fn define(&mut self, index: FuncIdx, body: Function) {
        let slot = self
            .funcs
            .get_mut((index.0 - self.imported_funcs) as usize)
            .expect("a reserved function");
        assert!(slot.1.is_none(), "function {} defined twice", index.0);
        slot.1 = Some(body);
    }

    /// Defines the function of a Python frame, which runs `prefix` before
    /// the code of `f`.
    pub fn define_frame(&mut self, index: FuncIdx, f: FuncState, prefix: &[u8]) {
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
            .push(((index.0 - self.imported_funcs) as usize, sites));
        self.define(index, f.finish_with_prefix(prefix));
    }

    pub fn add_func(&mut self, ty: TypeIdx, body: Function) -> FuncIdx {
        let index = self.reserve(ty);
        self.define(index, body);
        index
    }

    /// A memory with at least `bytes` bytes, exported as `export`.
    pub fn memory(&mut self, export: &str, bytes: i32) {
        let bytes = u64::try_from(bytes).expect("a positive size");
        self.memories.memory(MemoryType {
            minimum: bytes.div_ceil(PAGE),
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        self.exports.export(export, ExportKind::Memory, 0);
    }

    pub fn global(&mut self, ty: ValType, init: &ConstExpr) -> GlobalIdx {
        self.globals.global(
            GlobalType {
                val_type: ty,
                mutable: true,
                shared: false,
            },
            init,
        );
        self.global_count += 1;
        GlobalIdx(self.global_count - 1)
    }

    pub fn passive_data(&mut self, bytes: Vec<u8>) -> DataIdx {
        self.data.passive(bytes);
        self.data_count += 1;
        DataIdx(self.data_count - 1)
    }

    pub fn export_func(&mut self, name: &str, index: FuncIdx) {
        self.exports.export(name, ExportKind::Func, index.0);
    }

    /// The bytes of the module, with the place of the code of the Python
    /// frames. The function `module_frame` runs the frame of the module.
    pub fn finish(mut self, module_frame: FuncIdx) -> (Vec<u8>, CodeMap) {
        self.link_builtins();
        let mut functions = FunctionSection::new();
        let mut code = CodeSection::new();
        for (index, (ty, body)) in self.funcs.iter().enumerate() {
            functions.function(ty.0);
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
            (module_frame.0 - self.imported_funcs) as usize,
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

/// A block, loop or `if` that a branch can go to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Label(u32);

/// The locals and the code of a function while it is emitted. A local is
/// added when the code asks for one.
pub(crate) struct FuncState {
    params: u32,
    locals: Vec<ValType>,
    /// The temporaries that no code uses now.
    free: Vec<(ValType, LocalIdx)>,
    /// The label of each block that the code is in, the innermost last.
    blocks: Vec<Label>,
    labels: u32,
    code: Vec<u8>,
    sites: Vec<SiteRange>,
}

impl FuncState {
    pub fn new(params: u32) -> Self {
        FuncState {
            params,
            locals: Vec::new(),
            free: Vec::new(),
            blocks: Vec::new(),
            labels: 0,
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

    /// The parameter at `index`, the first parameter at 0.
    pub fn param(&self, index: u32) -> LocalIdx {
        assert!(index < self.params, "a parameter of the function");
        LocalIdx(index)
    }

    pub fn local(&mut self, ty: ValType) -> LocalIdx {
        self.locals.push(ty);
        LocalIdx(
            self.params + u32::try_from(self.locals.len() - 1).expect("fewer than 2^32 locals"),
        )
    }

    /// A local for a value that lives inside one expression, which goes
    /// back with `free`. Its value is whatever the last user left, so the
    /// code sets it before it reads it.
    pub fn temp(&mut self, ty: ValType) -> LocalIdx {
        match self.free.iter().position(|&(free, _)| free == ty) {
            Some(i) => self.free.swap_remove(i).1,
            None => self.local(ty),
        }
    }

    pub fn free(&mut self, local: LocalIdx) {
        let ty = local
            .0
            .checked_sub(self.params)
            .and_then(|i| self.locals.get(usize::try_from(i).ok()?))
            .expect("a temporary is a local of the function");
        self.free.push((*ty, local));
    }

    /// The sink for the instructions other than those of blocks and
    /// branches, which go through the methods of `FuncState` so that a
    /// branch names its target by a label.
    pub fn ins(&mut self) -> Ins<'_> {
        Ins::new(&mut self.code)
    }

    fn raw(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.code)
    }

    pub fn block(&mut self, ty: BlockType) -> Label {
        self.raw().block(ty);
        self.open()
    }

    pub fn loop_(&mut self, ty: BlockType) -> Label {
        self.raw().loop_(ty);
        self.open()
    }

    pub fn if_(&mut self, ty: BlockType) -> Label {
        self.raw().if_(ty);
        self.open()
    }

    /// Opens a `try_table` whose `catch` goes to `target` with the
    /// exception of `tag`.
    pub fn try_table(&mut self, ty: BlockType, tag: TagIdx, target: Label) -> Label {
        let label = self.depth(target);
        self.raw().try_table(ty, [Catch::One { tag: tag.0, label }]);
        self.open()
    }

    pub fn else_(&mut self) {
        self.raw().else_();
    }

    pub fn end(&mut self) {
        self.blocks.pop().expect("an end closes a block");
        self.raw().end();
    }

    pub fn br(&mut self, target: Label) {
        let depth = self.depth(target);
        self.raw().br(depth);
    }

    pub fn br_if(&mut self, target: Label) {
        let depth = self.depth(target);
        self.raw().br_if(depth);
    }

    fn open(&mut self) -> Label {
        let label = Label(self.labels);
        self.labels += 1;
        self.blocks.push(label);
        label
    }

    /// How many blocks the code is in inside `target`.
    fn depth(&self, target: Label) -> u32 {
        let inner = self
            .blocks
            .iter()
            .rev()
            .position(|&label| label == target)
            .expect("a branch to a block that the code is in");
        u32::try_from(inner).expect("fewer than 2^32 blocks")
    }

    /// The function, with `prefix` run before its own code. `prefix` uses
    /// no local.
    fn finish_with_prefix(self, prefix: &[u8]) -> Function {
        assert!(self.blocks.is_empty(), "a block without an end");
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

/// The sink for the instructions of a function, whose indices have the
/// type of what they index.
pub(crate) struct Ins<'a>(InstructionSink<'a>);

/// Methods of [`Ins`] that pass their arguments to the method of
/// [`InstructionSink`] with the same name.
macro_rules! same {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            pub fn $name(&mut self, $($arg: $ty),*) -> &mut Self {
                self.0.$name($($arg),*);
                self
            }
        )*
    };
}

impl<'a> Ins<'a> {
    /// The sink for the instructions in `code`.
    pub fn new(code: &'a mut Vec<u8>) -> Self {
        Ins(InstructionSink::new(code))
    }

    pub fn call(&mut self, func: FuncIdx) -> &mut Self {
        self.0.call(func.0);
        self
    }

    pub fn throw(&mut self, tag: TagIdx) -> &mut Self {
        self.0.throw(tag.0);
        self
    }

    pub fn local_get(&mut self, local: LocalIdx) -> &mut Self {
        self.0.local_get(local.0);
        self
    }

    pub fn local_set(&mut self, local: LocalIdx) -> &mut Self {
        self.0.local_set(local.0);
        self
    }

    pub fn local_tee(&mut self, local: LocalIdx) -> &mut Self {
        self.0.local_tee(local.0);
        self
    }

    pub fn global_get(&mut self, global: GlobalIdx) -> &mut Self {
        self.0.global_get(global.0);
        self
    }

    pub fn global_set(&mut self, global: GlobalIdx) -> &mut Self {
        self.0.global_set(global.0);
        self
    }

    pub fn data_drop(&mut self, data: DataIdx) -> &mut Self {
        self.0.data_drop(data.0);
        self
    }

    pub fn struct_new(&mut self, ty: TypeIdx) -> &mut Self {
        self.0.struct_new(ty.0);
        self
    }

    pub fn struct_get(&mut self, ty: TypeIdx, field: u32) -> &mut Self {
        self.0.struct_get(ty.0, field);
        self
    }

    pub fn struct_set(&mut self, ty: TypeIdx, field: u32) -> &mut Self {
        self.0.struct_set(ty.0, field);
        self
    }

    pub fn array_new_default(&mut self, ty: TypeIdx) -> &mut Self {
        self.0.array_new_default(ty.0);
        self
    }

    pub fn array_new_fixed(&mut self, ty: TypeIdx, len: u32) -> &mut Self {
        self.0.array_new_fixed(ty.0, len);
        self
    }

    pub fn array_new_data(&mut self, ty: TypeIdx, data: DataIdx) -> &mut Self {
        self.0.array_new_data(ty.0, data.0);
        self
    }

    pub fn array_get(&mut self, ty: TypeIdx) -> &mut Self {
        self.0.array_get(ty.0);
        self
    }

    pub fn array_get_u(&mut self, ty: TypeIdx) -> &mut Self {
        self.0.array_get_u(ty.0);
        self
    }

    pub fn array_set(&mut self, ty: TypeIdx) -> &mut Self {
        self.0.array_set(ty.0);
        self
    }

    pub fn array_copy(&mut self, dst: TypeIdx, src: TypeIdx) -> &mut Self {
        self.0.array_copy(dst.0, src.0);
        self
    }

    same! {
        array_len();
        drop();
        memory_copy(dst: u32, src: u32);
        ref_as_non_null();
        ref_is_null();
        ref_null(ty: HeapType);
        return_();
        select();
        unreachable();
        i32_const(value: i32);
        i32_load(arg: MemArg);
        i32_load8_u(arg: MemArg);
        i32_store(arg: MemArg);
        i32_store8(arg: MemArg);
        i32_add();
        i32_and();
        i32_eq();
        i32_eqz();
        i32_ge_s();
        i32_ge_u();
        i32_gt_s();
        i32_gt_u();
        i32_le_s();
        i32_le_u();
        i32_lt_s();
        i32_lt_u();
        i32_mul();
        i32_ne();
        i32_or();
        i32_rem_s();
        i32_shr_u();
        i32_sub();
        i32_trunc_f64_s();
        i32_wrap_i64();
        i32_xor();
        i64_const(value: i64);
        i64_add();
        i64_div_s();
        i64_div_u();
        i64_eq();
        i64_eqz();
        i64_extend_i32_s();
        i64_extend_i32_u();
        i64_ge_u();
        i64_gt_s();
        i64_gt_u();
        i64_lt_s();
        i64_lt_u();
        i64_mul();
        i64_ne();
        i64_rem_s();
        i64_rem_u();
        i64_shl();
        i64_sub();
        f64_const(value: Ieee64);
        f64_abs();
        f64_add();
        f64_convert_i32_s();
        f64_div();
        f64_eq();
        f64_ge();
        f64_gt();
        f64_le();
        f64_lt();
        f64_ne();
        f64_nearest();
        f64_mul();
        f64_neg();
        f64_sub();
    }
}
