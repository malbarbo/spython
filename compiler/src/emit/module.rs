//! The state of the module and the state of one function while they are
//! emitted.

use std::collections::HashMap;

use wasm_encoder::{
    CodeSection, ConstExpr, DataCountSection, DataSection, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, GlobalSection, GlobalType, ImportSection, InstructionSink,
    MemorySection, MemoryType, Module, StorageType, TypeSection, ValType,
};

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
    globals: GlobalSection,
    global_count: u32,
    exports: ExportSection,
    data: DataSection,
    data_count: u32,
    /// The code `_start` runs before the program, which fills the globals.
    pub init: Vec<u8>,
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
            globals: GlobalSection::new(),
            global_count: 0,
            exports: ExportSection::new(),
            data: DataSection::new(),
            data_count: 0,
            init: Vec::new(),
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

    pub fn finish(self) -> Vec<u8> {
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
            .section(&self.globals)
            .section(&self.exports)
            .section(&DataCountSection {
                count: self.data_count,
            })
            .section(&code)
            .section(&self.data);
        module.finish()
    }
}

/// The locals and the code of a function while it is emitted. A local is
/// added when the code asks for one.
pub(crate) struct FuncState {
    params: u32,
    locals: Vec<ValType>,
    code: Vec<u8>,
}

impl FuncState {
    pub fn new(params: u32) -> Self {
        FuncState {
            params,
            locals: Vec::new(),
            code: Vec::new(),
        }
    }

    pub fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        self.params + u32::try_from(self.locals.len() - 1).expect("fewer than 2^32 locals")
    }

    pub fn ins(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.code)
    }

    /// The function, with `prefix` run before its own code. `prefix` uses
    /// no local.
    pub fn finish_with_prefix(self, prefix: &[u8]) -> Function {
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
