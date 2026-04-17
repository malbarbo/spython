//! WASM code generation from Python AST.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use wasm_encoder::{
    AbstractHeapType, BlockType, CodeSection, ConstExpr, ExportKind, ExportSection, FieldType, Function,
    FunctionSection, GlobalSection, GlobalType, HeapType, IndirectNameMap, Instruction, Module,
    NameMap, NameSection, StorageType, TypeSection, ValType,
};

use crate::types::{resolve_annotation, WasmType};

pub fn compile_module(stmts: Vec<Stmt>) -> Result<Vec<u8>, String> {
    let mut c = Compiler::new();

    // Register some common array types
    c.register_array_type(WasmType::I64);
    c.register_array_type(WasmType::F64);
    c.register_array_type(WasmType::String);

    c.string_eq_idx = Some(c.emit_string_eq()?);

    // Pre-scan for classes (dataclasses and enums)
    for stmt in &stmts {
        if let Stmt::ClassDef(class) = stmt {
            let is_enum = class.bases().iter().any(|base| {
                if let Expr::Name(name) = base {
                    name.id == "Enum"
                } else {
                    false
                }
            });

            if is_enum {
                let mut members = HashMap::new();
                let mut next_val = 1i64;
                for item in &class.body {
                    if let Stmt::Assign(assign) = item {
                        for target in &assign.targets {
                            if let Expr::Name(name) = target {
                                members.insert(name.id.to_string(), next_val);
                                next_val += 1;
                            }
                        }
                    }
                }
                c.register_enum_type(&class.name, members);
            } else {
                let mut fields = Vec::new();
                for item in &class.body {
                    if let Stmt::AnnAssign(assign) = item {
                        if let Expr::Name(target) = assign.target.as_ref() {
                            fields.push((
                                target.id.to_string(),
                                resolve_annotation(&assign.annotation)?,
                            ));
                        }
                    }
                }
                c.register_struct_type(&class.name, fields)?;
            }
        }
    }

    // Collect all types from annotations to register array types
    for stmt in &stmts {
        c.collect_types_stmt(stmt, None)?;
    }

    // First pass: register globals and compile functions
    for stmt in &stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                c.compile_function(func, None)?;
            }
            Stmt::ClassDef(class) => {
                let is_enum = class.bases().iter().any(|base| {
                    if let Expr::Name(name) = base {
                        name.id == "Enum"
                    } else {
                        false
                    }
                });

                if is_enum {
                    let ei = c.enum_types.get(class.name.as_str()).cloned().unwrap();
                    for item in &class.body {
                        if let Stmt::Assign(assign) = item {
                            for target in &assign.targets {
                                if let Expr::Name(name) = target {
                                    let full_name = format!("{}.{}", class.name, name.id);
                                    c.add_global(&full_name, WasmType::Struct(class.name.to_string()));
                                }
                            }
                        }
                    }
                } else {
                    let mut method_indices = HashMap::new();
                    for item in &class.body {
                        if let Stmt::FunctionDef(func) = item {
                            let idx = c.compile_function(func, Some(&class.name))?;
                            method_indices.insert(func.name.to_string(), idx);
                        }
                    }
                    if let Some(si) = c.struct_types.get_mut(class.name.as_str()) {
                        si.methods = method_indices;
                    }
                }
            }
            Stmt::AnnAssign(assign) => {
                if let Expr::Name(name) = assign.target.as_ref() {
                    c.add_global(&name.id, resolve_annotation(&assign.annotation)?);
                }
            }
            _ => {}
        }
    }

    // Second pass: top-level statements → _start
    let top: Vec<&Stmt> = stmts
        .iter()
        .filter(|s| !matches!(s, Stmt::FunctionDef(_)))
        .collect();
    if !top.is_empty() {
        c.compile_start(&top)?;
    }

    Ok(c.finish())
}

// =====================================================================

struct FuncInfo {
    name: String,
    param_types: Vec<WasmType>,
    return_type: WasmType,
    type_idx: u32,
    body: Option<Function>,
}

#[derive(Clone)]
struct GlobalInfo {
    name: String,
    wasm_type: WasmType,
    index: u32,
}

#[derive(Clone)]
struct StructInfo {
    name: String,
    fields: Vec<(String, WasmType)>,
    methods: HashMap<String, u32>, // method name -> function index
    type_idx: u32,
}

#[derive(Clone)]
struct EnumInfo {
    name: String,
    members: HashMap<String, i64>,
    type_idx: u32,
}

struct Compiler {
    types: TypeSection,
    type_count: u32,
    functions: FunctionSection,
    exports: ExportSection,
    codes: CodeSection,
    globals_sec: GlobalSection,
    global_infos: Vec<GlobalInfo>,
    func_infos: Vec<FuncInfo>,
    func_names: NameMap,
    func_locals: IndirectNameMap,
    array_types: HashMap<WasmType, u32>,
    list_types: HashMap<WasmType, u32>,
    dict_types: HashMap<(WasmType, WasmType), u32>,
    set_types: HashMap<WasmType, u32>,
    struct_types: HashMap<String, StructInfo>,
    enum_types: HashMap<String, EnumInfo>,
    signature_types: HashMap<(Vec<WasmType>, WasmType), u32>,
    lambda_count: u32,
    string_eq_idx: Option<u32>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            types: TypeSection::new(),
            type_count: 0,
            functions: FunctionSection::new(),
            exports: ExportSection::new(),
            codes: CodeSection::new(),
            globals_sec: GlobalSection::new(),
            global_infos: Vec::new(),
            func_infos: Vec::new(),
            func_names: NameMap::new(),
            func_locals: IndirectNameMap::new(),
            array_types: HashMap::new(),
            list_types: HashMap::new(),
            dict_types: HashMap::new(),
            set_types: HashMap::new(),
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
            signature_types: HashMap::new(),
            lambda_count: 0,
            string_eq_idx: None,
        }
    }

    fn register_array_type(&mut self, elem_type: WasmType) {
        if self.array_types.contains_key(&elem_type) {
            return;
        }
        let val_type = self.wasm_type_to_val_type_pure(&elem_type);
        let array_idx = self.type_count;
        self.types.ty().array(
            &StorageType::Val(val_type),
            true,
        );
        self.type_count += 1;
        self.array_types.insert(elem_type.clone(), array_idx);

        let list_idx = self.type_count;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(wasm_encoder::RefType {
                    nullable: true,
                    heap_type: wasm_encoder::HeapType::Concrete(array_idx),
                })),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: true,
            },
        ]);
        self.type_count += 1;
        self.list_types.insert(elem_type, list_idx);
    }

    fn register_dict_type(&mut self, key_type: WasmType, val_type: WasmType) {
        let key = (key_type.clone(), val_type.clone());
        if self.dict_types.contains_key(&key) {
            return;
        }
        self.register_array_type(key_type.clone());
        self.register_array_type(val_type.clone());
        let key_array_idx = *self.array_types.get(&key_type).unwrap();
        let val_array_idx = *self.array_types.get(&val_type).unwrap();

        let dict_idx = self.type_count;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(wasm_encoder::RefType {
                    nullable: true,
                    heap_type: wasm_encoder::HeapType::Concrete(key_array_idx),
                })),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(ValType::Ref(wasm_encoder::RefType {
                    nullable: true,
                    heap_type: wasm_encoder::HeapType::Concrete(val_array_idx),
                })),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: true,
            },
        ]);
        self.type_count += 1;
        self.dict_types.insert(key, dict_idx);
    }

    fn register_set_type(&mut self, elem_type: WasmType) {
        if self.set_types.contains_key(&elem_type) {
            return;
        }
        self.register_array_type(elem_type.clone());
        let array_idx = *self.array_types.get(&elem_type).unwrap();

        let set_idx = self.type_count;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::Ref(wasm_encoder::RefType {
                    nullable: true,
                    heap_type: wasm_encoder::HeapType::Concrete(array_idx),
                })),
                mutable: true,
            },
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: true,
            },
        ]);
        self.type_count += 1;
        self.set_types.insert(elem_type, set_idx);
    }

    fn emit_string_eq(&mut self) -> Result<u32, String> {
        let string_type = self.array_types.get(&WasmType::I64).copied().unwrap();
        let ref_string = ValType::Ref(wasm_encoder::RefType {
            nullable: true,
            heap_type: wasm_encoder::HeapType::Concrete(string_type),
        });

        let idx = self.func_infos.len() as u32;
        self.types.ty().function(vec![ref_string, ref_string], vec![ValType::I32]);
        let type_idx = self.type_count;
        self.type_count += 1;

        self.func_names.append(idx, "string_eq");
        self.func_infos.push(FuncInfo {
            name: "string_eq".to_string(),
            param_types: vec![WasmType::String, WasmType::String],
            return_type: WasmType::I64, // We'll extend i32 to i64 in the caller if needed, but here it returns i32
            type_idx,
            body: None,
        });

        let mut body = Vec::new();
        // Param 0: s1, Param 1: s2
        
        // if s1 == s2 return True (RefEq)
        body.push(Instruction::LocalGet(0));
        body.push(Instruction::LocalGet(1));
        body.push(Instruction::RefEq);
        body.push(Instruction::If(BlockType::Empty));
        body.push(Instruction::I32Const(1));
        body.push(Instruction::Return);
        body.push(Instruction::End);

        // if s1 == null or s2 == null return False
        body.push(Instruction::LocalGet(0));
        body.push(Instruction::RefIsNull);
        body.push(Instruction::LocalGet(1));
        body.push(Instruction::RefIsNull);
        body.push(Instruction::I32Or);
        body.push(Instruction::If(BlockType::Empty));
        body.push(Instruction::I32Const(0));
        body.push(Instruction::Return);
        body.push(Instruction::End);

        // if len(s1) != len(s2) return False
        body.push(Instruction::LocalGet(0));
        body.push(Instruction::ArrayLen);
        body.push(Instruction::LocalGet(1));
        body.push(Instruction::ArrayLen);
        body.push(Instruction::I32Ne);
        body.push(Instruction::If(BlockType::Empty));
        body.push(Instruction::I32Const(0));
        body.push(Instruction::Return);
        body.push(Instruction::End);

        // Loop and compare chars
        let i_tmp = 2u32;
        let len_tmp = 3u32;
        let mut locals = Vec::new();
        locals.push((1, ValType::I32)); // i
        locals.push((1, ValType::I32)); // len
        
        body.push(Instruction::LocalGet(0));
        body.push(Instruction::ArrayLen);
        body.push(Instruction::LocalSet(len_tmp));
        body.push(Instruction::I32Const(0));
        body.push(Instruction::LocalSet(i_tmp));

        body.push(Instruction::Block(BlockType::Empty));
        body.push(Instruction::Loop(BlockType::Empty));
        // i < len
        body.push(Instruction::LocalGet(i_tmp));
        body.push(Instruction::LocalGet(len_tmp));
        body.push(Instruction::I32GeU);
        body.push(Instruction::BrIf(1));

        // if s1[i] != s2[i] return False
        body.push(Instruction::LocalGet(0));
        body.push(Instruction::LocalGet(i_tmp));
        body.push(Instruction::ArrayGet(string_type));
        body.push(Instruction::LocalGet(1));
        body.push(Instruction::LocalGet(i_tmp));
        body.push(Instruction::ArrayGet(string_type));
        body.push(Instruction::I64Ne);
        body.push(Instruction::If(BlockType::Empty));
        body.push(Instruction::I32Const(0));
        body.push(Instruction::Return);
        body.push(Instruction::End);

        // i += 1
        body.push(Instruction::LocalGet(i_tmp));
        body.push(Instruction::I32Const(1));
        body.push(Instruction::I32Add);
        body.push(Instruction::LocalSet(i_tmp));
        body.push(Instruction::Br(0));
        body.push(Instruction::End);
        body.push(Instruction::End);

        body.push(Instruction::I32Const(1));
        body.push(Instruction::End);

        let mut f = Function::new(locals);
        for i in &body {
            f.instruction(i);
        }
        self.func_infos[idx as usize].body = Some(f);
        
        // Add to func_locals for debug names
        let mut nm = NameMap::new();
        nm.append(0, "s1");
        nm.append(1, "s2");
        nm.append(2, "i");
        nm.append(3, "len");
        self.func_locals.append(idx, &nm);

        Ok(idx)
    }

    fn register_struct_type(
        &mut self,
        name: &str,
        fields: Vec<(String, WasmType)>,
    ) -> Result<(), String> {
        if self.struct_types.contains_key(name) {
            return Ok(());
        }
        let type_idx = self.type_count;
        let mut wasm_fields = Vec::new();
        for (_, wt) in &fields {
            wasm_fields.push(FieldType {
                element_type: StorageType::Val(self.wasm_type_to_val_type(wt)),
                mutable: true,
            });
        }
        self.types.ty().struct_(wasm_fields); // Use wasm_fields by value
        self.type_count += 1;
        self.struct_types.insert(
            name.to_string(),
            StructInfo {
                name: name.to_string(),
                fields,
                methods: HashMap::new(),
                type_idx,
            },
        );
        self.register_array_type(WasmType::Struct(name.to_string()));
        Ok(())
    }

    fn collect_types_stmt(&mut self, stmt: &Stmt, class_name: Option<&str>) -> Result<(), String> {
        match stmt {
            Stmt::FunctionDef(func) => {
                for arg in &func.parameters.args {
                    let name = arg.parameter.name.as_str();
                    let ann = if name == "self" && arg.parameter.annotation.is_none() {
                        if let Some(cls) = class_name {
                            WasmType::Struct(cls.to_string())
                        } else {
                            return Err("`self` parameter outside of class".to_string());
                        }
                    } else {
                        resolve_annotation(arg.parameter.annotation.as_ref().unwrap())?
                    };
                    self.register_type(&ann);
                }
                if let Some(returns) = &func.returns {
                    self.register_type(&resolve_annotation(returns)?);
                }
                for s in &func.body {
                    self.collect_types_stmt(s, class_name)?;
                }
            }
            Stmt::ClassDef(class) => {
                for item in &class.body {
                    self.collect_types_stmt(item, Some(&class.name))?;
                }
            }
            Stmt::AnnAssign(assign) => {
                self.register_type(&resolve_annotation(&assign.annotation)?);
                if let Some(value) = &assign.value {
                    self.collect_types_expr(value)?;
                }
            }
            Stmt::Assign(assign) => {
                self.collect_types_expr(&assign.value)?;
            }
            Stmt::AugAssign(assign) => {
                self.collect_types_expr(&assign.value)?;
            }
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_types_expr(value)?;
                }
            }
            Stmt::If(i) => {
                self.collect_types_expr(&i.test)?;
                for s in &i.body {
                    self.collect_types_stmt(s, class_name)?;
                }
                for clause in &i.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        self.collect_types_expr(test)?;
                    }
                    for s in &clause.body {
                        self.collect_types_stmt(s, class_name)?;
                    }
                }
            }
            Stmt::While(w) => {
                self.collect_types_expr(&w.test)?;
                for s in &w.body {
                    self.collect_types_stmt(s, class_name)?;
                }
                for s in &w.orelse {
                    self.collect_types_stmt(s, class_name)?;
                }
            }
            Stmt::For(f) => {
                self.collect_types_expr(&f.iter)?;
                for s in &f.body {
                    self.collect_types_stmt(s, class_name)?;
                }
                for s in &f.orelse {
                    self.collect_types_stmt(s, class_name)?;
                }
            }
            Stmt::Expr(e) => {
                self.collect_types_expr(&e.value)?;
            }
            Stmt::Assert(a) => {
                self.collect_types_expr(&a.test)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn collect_types_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::List(list) => {
                if !list.elts.is_empty() {
                    let et = self.infer_type(&list.elts[0], &[]);
                    self.register_type(&WasmType::List(Box::new(et)));
                }
                for elt in &list.elts {
                    self.collect_types_expr(elt)?;
                }
            }
            Expr::Dict(dict) => {
                if !dict.items.is_empty() {
                    let kt = self.infer_type(dict.items[0].key.as_ref().unwrap(), &[]);
                    let vt = self.infer_type(&dict.items[0].value, &[]);
                    self.register_type(&WasmType::Dict(Box::new(kt), Box::new(vt)));
                }
                for item in &dict.items {
                    self.collect_types_expr(item.key.as_ref().unwrap())?;
                    self.collect_types_expr(&item.value)?;
                }
            }
            Expr::Set(set) => {
                if !set.elts.is_empty() {
                    let et = self.infer_type(&set.elts[0], &[]);
                    self.register_type(&WasmType::Set(Box::new(et)));
                }
                for elt in &set.elts {
                    self.collect_types_expr(elt)?;
                }
            }
            Expr::ListComp(lc) => {
                // For ListComp, we need to register the resulting list type
                // infer_type for ListComp already handles the logic
                let lt = self.infer_type(expr, &[]);
                self.register_type(&lt);
                
                self.collect_types_expr(&lc.elt)?;
                for generator in &lc.generators {
                    self.collect_types_expr(&generator.iter)?;
                    for if_expr in &generator.ifs {
                        self.collect_types_expr(if_expr)?;
                    }
                }
            }
            Expr::Call(call) => {
                self.collect_types_expr(&call.func)?;
                for arg in &call.arguments.args {
                    self.collect_types_expr(arg)?;
                }
            }
            Expr::BinOp(b) => {
                self.collect_types_expr(&b.left)?;
                self.collect_types_expr(&b.right)?;
            }
            Expr::UnaryOp(u) => {
                self.collect_types_expr(&u.operand)?;
            }
            Expr::Compare(c) => {
                self.collect_types_expr(&c.left)?;
                for cmp in &c.comparators {
                    self.collect_types_expr(cmp)?;
                }
            }
            Expr::Attribute(a) => {
                self.collect_types_expr(&a.value)?;
            }
            Expr::Subscript(s) => {
                self.collect_types_expr(&s.value)?;
                self.collect_types_expr(&s.slice)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn register_type(&mut self, wt: &WasmType) {
        match wt {
            WasmType::Array(et) => {
                self.register_array_type(et.as_ref().clone());
                self.register_type(et);
            }
            WasmType::List(et) => {
                self.register_array_type(et.as_ref().clone());
                self.register_type(et);
            }
            WasmType::Dict(kt, vt) => {
                self.register_dict_type(kt.as_ref().clone(), vt.as_ref().clone());
                self.register_type(kt);
                self.register_type(vt);
            }
            WasmType::Set(et) => {
                self.register_set_type(et.as_ref().clone());
                self.register_type(et);
            }
            WasmType::Callable { params, ret } => {
                for p in params {
                    self.register_type(p);
                }
                self.register_type(ret);
                self.get_signature_type_idx(params, ret);
            }
            _ => {}
        }
    }

    fn register_enum_type(&mut self, name: &str, members: HashMap<String, i64>) {
        let string_vt = self.wasm_type_to_val_type_pure(&WasmType::String);
        let type_idx = self.type_count;
        self.types.ty().struct_(vec![
            FieldType {
                element_type: StorageType::Val(ValType::I64),
                mutable: false,
            },
            FieldType {
                element_type: StorageType::Val(string_vt),
                mutable: false,
            },
        ]);
        self.type_count += 1;

        self.enum_types.insert(
            name.to_string(),
            EnumInfo {
                name: name.to_string(),
                members,
                type_idx,
            },
        );
    }

    fn get_signature_type_idx(&mut self, params: &[WasmType], ret: &WasmType) -> u32 {
        let key = (params.to_vec(), ret.clone());
        if let Some(&idx) = self.signature_types.get(&key) {
            return idx;
        }
        let mut wasm_params = Vec::new();
        for p in params {
            wasm_params.push(self.wasm_type_to_val_type_pure(p));
        }
        let wasm_ret = self.wasm_type_to_val_type_pure(ret);
        self.types.ty().function(wasm_params, vec![wasm_ret]);
        let idx = self.type_count;
        self.type_count += 1;
        self.signature_types.insert(key, idx);
        idx
    }

    fn wasm_type_to_val_type(&mut self, wt: &WasmType) -> ValType {
        match wt {
            WasmType::I32 => ValType::I32,
            WasmType::String => {
                if let Some(&idx) = self.array_types.get(&WasmType::I64) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Struct(name) => {
                if let Some(ei) = self.enum_types.get(name) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(ei.type_idx),
                    })
                } else if let Some(si) = self.struct_types.get(name) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(si.type_idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Array(et) => {
                if let Some(&idx) = self.array_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::List(et) => {
                if let Some(&idx) = self.list_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Dict(kt, vt) => {
                if let Some(&idx) = self.dict_types.get(&(kt.as_ref().clone(), vt.as_ref().clone())) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Set(et) => {
                if let Some(&idx) = self.set_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Callable { params, ret } => {
                let idx = self.get_signature_type_idx(params, ret);
                ValType::Ref(wasm_encoder::RefType {
                    nullable: true,
                    heap_type: wasm_encoder::HeapType::Concrete(idx),
                })
            }
            _ => wt.clone().to_val_type(),
        }
    }

    fn wasm_type_to_val_type_pure(&self, wt: &WasmType) -> ValType {
        match wt {
            WasmType::I32 => ValType::I32,
            WasmType::String => {
                if let Some(&idx) = self.array_types.get(&WasmType::I64) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Struct(name) => {
                if let Some(ei) = self.enum_types.get(name) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(ei.type_idx),
                    })
                } else if let Some(si) = self.struct_types.get(name) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(si.type_idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Array(et) => {
                if let Some(&idx) = self.array_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::List(et) => {
                if let Some(&idx) = self.list_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Dict(kt, vt) => {
                if let Some(&idx) = self.dict_types.get(&(kt.as_ref().clone(), vt.as_ref().clone())) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Set(et) => {
                if let Some(&idx) = self.set_types.get(et) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    wt.clone().to_val_type()
                }
            }
            WasmType::Callable { params, ret } => {
                let key = (params.clone(), *ret.clone());
                if let Some(&idx) = self.signature_types.get(&key) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Concrete(idx),
                    })
                } else {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: wasm_encoder::HeapType::Abstract {
                            shared: false,
                            ty: wasm_encoder::AbstractHeapType::Func,
                        },
                    })
                }
            }
            _ => wt.clone().to_val_type(),
        }
    }

    fn add_global(&mut self, name: &str, wasm_type: WasmType) {
        let index = self.global_infos.len() as u32;
        let init = match wasm_type {
            WasmType::I64 => ConstExpr::i64_const(0),
            WasmType::I32 => ConstExpr::i32_const(0),
            WasmType::F64 => ConstExpr::f64_const(0.0f64.into()),
            WasmType::String => {
                if let Some(&idx) = self.array_types.get(&WasmType::I64) {
                    ConstExpr::ref_null(HeapType::Concrete(idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    })
                }
            }
            WasmType::Array(ref et) => {
                if let Some(&idx) = self.array_types.get(et) {
                    ConstExpr::ref_null(HeapType::Concrete(idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    })
                }
            }
            WasmType::List(ref et) => {
                if let Some(&idx) = self.list_types.get(et) {
                    ConstExpr::ref_null(HeapType::Concrete(idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    })
                }
            }
            WasmType::Dict(ref kt, ref vt) => {
                if let Some(&idx) = self.dict_types.get(&(kt.as_ref().clone(), vt.as_ref().clone())) {
                    ConstExpr::ref_null(HeapType::Concrete(idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    })
                }
            }
            WasmType::Set(ref et) => {
                if let Some(&idx) = self.set_types.get(et) {
                    ConstExpr::ref_null(HeapType::Concrete(idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    })
                }
            }
            WasmType::Struct(ref name) => {
                if let Some(ei) = self.enum_types.get(name) {
                    ConstExpr::ref_null(HeapType::Concrete(ei.type_idx))
                } else if let Some(si) = self.struct_types.get(name) {
                    ConstExpr::ref_null(HeapType::Concrete(si.type_idx))
                } else {
                    ConstExpr::ref_null(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Struct,
                    })
                }
            }
            WasmType::None => ConstExpr::ref_null(HeapType::Abstract {
                shared: false,
                ty: AbstractHeapType::None,
            }),
            WasmType::Callable { ref params, ref ret } => {
                let idx = self.get_signature_type_idx(params, ret);
                ConstExpr::ref_null(HeapType::Concrete(idx))
            }
        };
        let vt = self.wasm_type_to_val_type(&wasm_type);
        self.globals_sec.global(
            GlobalType {
                val_type: vt,
                mutable: true,
                shared: false,
            },
            &init,
        );
        self.global_infos.push(GlobalInfo {
            name: name.to_string(),
            wasm_type,
            index,
        });
    }

    fn compile_func_low_level(
        &mut self,
        name: String,
        param_names: Vec<String>,
        param_types: Vec<WasmType>,
        ret_type: WasmType,
        body_expr: Option<&Expr>,
        body_stmts: Option<&[Stmt]>,
    ) -> Result<u32, String> {
        let ret_val = self.wasm_type_to_val_type(&ret_type);
        let idx = self.func_infos.len() as u32;
        let wasm_params: Vec<ValType> = param_types
            .iter()
            .map(|t| self.wasm_type_to_val_type(t))
            .collect();

        self.types.ty().function(wasm_params, vec![ret_val]);
        let type_idx = self.type_count;
        self.type_count += 1;

        self.exports.export(&name, ExportKind::Func, idx);
        self.func_names.append(idx, &name);
        self.func_infos.push(FuncInfo {
            name: name.clone(),
            param_types: param_types.clone(),
            return_type: ret_type.clone(),
            type_idx,
            body: None,
        });

        let mut locals = Vec::new();
        let mut names: Vec<(String, WasmType)> = Vec::new();
        for (i, p_name) in param_names.iter().enumerate() {
            names.push((p_name.clone(), param_types[i].clone()));
        }

        let mut gs: Vec<String> = Vec::new();
        let mut body = Vec::new();

        if let Some(expr) = body_expr {
            self.compile_expr(expr, None, &mut body, &mut locals, &mut names, &mut gs)?;
        } else if let Some(stmts) = body_stmts {
            for stmt in stmts {
                self.compile_stmt(stmt, &ret_type, &mut body, &mut locals, &mut names, &mut gs)?;
            }
            self.emit_default_value(&ret_type, &mut body)?;
        }

        body.push(Instruction::End);

        let mut f = Function::new(locals);
        for i in &body {
            f.instruction(i);
        }
        self.func_infos[idx as usize].body = Some(f);

        let mut nm = NameMap::new();
        for (i, (n, _)) in names.iter().enumerate() {
            nm.append(i as u32, n);
        }
        self.func_locals.append(idx, &nm);
        Ok(idx)
    }

    fn find_global(&self, name: &str) -> Option<&GlobalInfo> {
        self.global_infos.iter().find(|g| g.name == name)
    }

    fn emit_default_value<'a>(
        &mut self,
        wt: &WasmType,
        body: &mut Vec<Instruction<'a>>,
    ) -> Result<(), String> {
        match wt {
            WasmType::I64 => body.push(Instruction::I64Const(0)),
            WasmType::I32 => body.push(Instruction::I32Const(0)),
            WasmType::F64 => body.push(Instruction::F64Const(0.0f64.into())),
            WasmType::String => {
                if let Some(&idx) = self.array_types.get(&WasmType::I64) {
                    body.push(Instruction::RefNull(HeapType::Concrete(idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    }));
                }
            }
            WasmType::Array(et) => {
                if let Some(&idx) = self.array_types.get(et) {
                    body.push(Instruction::RefNull(HeapType::Concrete(idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    }));
                }
            }
            WasmType::List(et) => {
                if let Some(&idx) = self.list_types.get(et) {
                    body.push(Instruction::RefNull(HeapType::Concrete(idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    }));
                }
            }
            WasmType::Dict(kt, vt) => {
                if let Some(&idx) = self.dict_types.get(&(kt.as_ref().clone(), vt.as_ref().clone())) {
                    body.push(Instruction::RefNull(HeapType::Concrete(idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    }));
                }
            }
            WasmType::Set(et) => {
                if let Some(&idx) = self.set_types.get(et) {
                    body.push(Instruction::RefNull(HeapType::Concrete(idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Array,
                    }));
                }
            }
            WasmType::Struct(name) => {
                if let Some(ei) = self.enum_types.get(name) {
                    body.push(Instruction::RefNull(HeapType::Concrete(ei.type_idx)));
                } else if let Some(si) = self.struct_types.get(name) {
                    body.push(Instruction::RefNull(HeapType::Concrete(si.type_idx)));
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::Struct,
                    }));
                }
            }
            WasmType::None => body.push(Instruction::RefNull(HeapType::Abstract {
                shared: false,
                ty: AbstractHeapType::None,
            })),
            WasmType::Callable { params, ret } => {
                let idx = self.get_signature_type_idx(params, ret);
                body.push(Instruction::RefNull(HeapType::Concrete(idx)));
            }
        }
        Ok(())
    }

    fn compile_function(&mut self, func: &StmtFunctionDef, class_name: Option<&str>) -> Result<u32, String> {
        let params = &func.parameters;
        let mut param_names = Vec::new();
        let mut param_types = Vec::new();
        for p in params.posonlyargs.iter().chain(params.args.iter()) {
            let name = p.parameter.name.to_string();
            let ann_type = if name == "self" && p.parameter.annotation.is_none() {
                if let Some(cls) = class_name {
                    WasmType::Struct(cls.to_string())
                } else {
                    return Err("Missing annotation for `self`".to_string());
                }
            } else {
                let ann = p
                    .parameter
                    .annotation
                    .as_ref()
                    .ok_or_else(|| format!("Missing annotation for `{}`", p.parameter.name))?;
                resolve_annotation(ann)?
            };
            param_names.push(name);
            param_types.push(ann_type);
        }
        let ret = resolve_annotation(
            func.returns
                .as_ref()
                .ok_or_else(|| format!("Missing return annotation for `{}`", func.name))?,
        )?;

        let full_name = match class_name {
            Some(cls) => format!("{}.{}", cls, func.name),
            None => func.name.to_string(),
        };

        self.compile_func_low_level(full_name, param_names, param_types, ret, None, Some(&func.body))
    }

    fn compile_start(&mut self, stmts: &[&Stmt]) -> Result<(), String> {
        let ret_val = self.wasm_type_to_val_type(&WasmType::None);
        self.types.ty().function(vec![], vec![ret_val]);
        let type_idx = self.type_count;
        self.type_count += 1;

        let mut locals = Vec::new();
        let mut names: Vec<(String, WasmType)> = Vec::new();
        let mut gs: Vec<String> = Vec::new();
        let mut body = Vec::new();

        // Initialize Enum globals
        for stmt in stmts {
            if let Stmt::ClassDef(class) = stmt {
                let is_enum = class.bases().iter().any(|base| {
                    if let Expr::Name(name) = base {
                        name.id == "Enum"
                    } else {
                        false
                    }
                });
                if is_enum {
                    let ei = self.enum_types.get(class.name.as_str()).cloned().unwrap();
                    for item in &class.body {
                        if let Stmt::Assign(assign) = item {
                            for target in &assign.targets {
                                if let Expr::Name(name) = target {
                                    let full_name = format!("{}.{}", class.name, name.id);
                                    let gi = self.find_global(&full_name).cloned().unwrap();
                                    let val = ei.members.get(name.id.as_str()).cloned().unwrap();
                                    
                                    body.push(Instruction::I64Const(val));
                                    // Push the name as a string literal
                                    let string_array_idx = self.array_types.get(&WasmType::I64).copied().unwrap();
                                    for c in name.id.as_str().chars() {
                                        body.push(Instruction::I64Const(c as i64));
                                    }
                                    body.push(Instruction::ArrayNewFixed {
                                        array_type_index: string_array_idx,
                                        array_size: name.id.as_str().chars().count() as u32,
                                    });
                                    body.push(Instruction::StructNew(ei.type_idx));
                                    body.push(Instruction::GlobalSet(gi.index));
                                }
                            }
                        }
                    }
                }
            }
        }

        for stmt in stmts {
            self.compile_stmt(stmt, &WasmType::None, &mut body, &mut locals, &mut names, &mut gs)?;
        }
        body.push(Instruction::RefNull(HeapType::Abstract {
            shared: false,
            ty: AbstractHeapType::None,
        }));
        body.push(Instruction::End);

        let mut f = Function::new(locals);
        for i in &body {
            f.instruction(i);
        }

        let idx = self.func_infos.len() as u32;
        self.exports
            .export("_start", ExportKind::Func, idx);
        self.func_names.append(idx, "_start");

        self.func_infos.push(FuncInfo {
            name: "_start".to_string(),
            param_types: vec![],
            return_type: WasmType::None,
            type_idx,
            body: Some(f),
        });

        let mut nm = NameMap::new();
        for (i, (n, _)) in names.iter().enumerate() {
            nm.append(i as u32, n);
        }
        self.func_locals.append(idx, &nm);
        Ok(())
    }

    fn compile_stmt<'a>(
        &mut self,
        stmt: &Stmt,
        ret_type: &WasmType,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        match stmt {
            Stmt::Global(g) => {
                for n in &g.names {
                    gs.push(n.to_string());
                }
                Ok(())
            }
            Stmt::Return(ret) => {
                if let Some(v) = &ret.value {
                    self.compile_expr(v, Some(ret_type), body, locals, names, gs)?;
                    let vt = self.infer_type(v, names);
                    self.emit_coercion(body, vt, ret_type.clone());
                } else {
                    body.push(Instruction::RefNull(HeapType::Abstract {
                        shared: false,
                        ty: AbstractHeapType::None,
                    }));
                }
                body.push(Instruction::Return);
                Ok(())
            }
            Stmt::AnnAssign(assign) => {
                if let Expr::Name(name) = assign.target.as_ref() {
                    if let Some(gi) = self.find_global(&name.id).cloned() {
                        if let Some(v) = &assign.value {
                            self.compile_expr(v, Some(&gi.wasm_type), body, locals, names, gs)?;
                            let vt = self.infer_type(v, names);
                            self.emit_coercion(body, vt, gi.wasm_type);
                            body.push(Instruction::GlobalSet(gi.index));
                        }
                        return Ok(());
                    }
                    let wt = resolve_annotation(&assign.annotation)?;
                    let li = names.len() as u32;
                    locals.push((1, self.wasm_type_to_val_type(&wt)));
                    names.push((name.id.to_string(), wt.clone()));
                    if let Some(v) = &assign.value {
                        self.compile_expr(v, Some(&wt), body, locals, names, gs)?;
                        let vt = self.infer_type(v, names);
                        self.emit_coercion(body, vt, wt);
                    } else {
                        self.emit_default_value(&wt, body)?;
                    }
                    body.push(Instruction::LocalSet(li));
                    Ok(())
                } else if let Expr::Attribute(attr) = assign.target.as_ref() {
                    let wt = resolve_annotation(&assign.annotation)?;
                    if let Some(v) = &assign.value {
                        let obj_type = self.infer_type(&attr.value, names);
                        if let WasmType::Struct(struct_name) = obj_type {
                            let si = self.struct_types.get(&struct_name).cloned().ok_or_else(|| {
                                format!("Unknown struct: {}", struct_name)
                            })?;
                            let field_idx = si
                                .fields
                                .iter()
                                .position(|(n, _)| n == attr.attr.as_str())
                                .ok_or_else(|| format!("Unknown field: {}", attr.attr))?;
                            
                            self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                            self.compile_expr(v, None, body, locals, names, gs)?;
                            let vt = self.infer_type(v, names);
                            self.emit_coercion(body, vt, wt);
                            body.push(Instruction::StructSet {
                                struct_type_index: si.type_idx,
                                field_index: field_idx as u32,
                            });
                            Ok(())
                        } else {
                            Err("Attributes only supported on structs".to_string())
                        }
                    } else {
                        Err("Variable declaration without value".to_string())
                    }
                } else {
                    Err("Unsupported assignment target".to_string())
                }
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Expr::Name(name) = &assign.targets[0] {
                        if let Some(gi) = self.find_global(&name.id).cloned() {
                            if gs.contains(&name.id.to_string())
                                || !names.iter().any(|(n, _)| n == name.id.as_str())
                            {
                                self.compile_expr(&assign.value, None, body, locals, names, gs)?;
                                let vt = self.infer_type(&assign.value, names);
                                self.emit_coercion(body, vt, gi.wasm_type);
                                body.push(Instruction::GlobalSet(gi.index));
                                return Ok(());
                            }
                        }
                        if let Some(li) = names.iter().position(|(n, _)| n == name.id.as_str()) {
                            let wt = names[li].1.clone();
                            self.compile_expr(&assign.value, None, body, locals, names, gs)?;
                            let vt = self.infer_type(&assign.value, names);
                            self.emit_coercion(body, vt, wt);
                            body.push(Instruction::LocalSet(li as u32));
                            return Ok(());
                        }

                        // New local
                        let vt = self.infer_type(&assign.value, names);
                        let li = names.len() as u32;
                        locals.push((1, self.wasm_type_to_val_type(&vt)));
                        names.push((name.id.to_string(), vt.clone()));
                        self.compile_expr(&assign.value, Some(&vt), body, locals, names, gs)?;
                        body.push(Instruction::LocalSet(li));
                        return Ok(());
                    } else if let Expr::Attribute(attr) = &assign.targets[0] {
                        let obj_type = self.infer_type(&attr.value, names);
                        if let WasmType::Struct(struct_name) = obj_type {
                            let si = self.struct_types.get(&struct_name).cloned().ok_or_else(|| {
                                format!("Unknown struct: {}", struct_name)
                            })?;
                            let field_idx = si
                                .fields
                                .iter()
                                .position(|(n, _)| n == attr.attr.as_str())
                                .ok_or_else(|| format!("Unknown field: {}", attr.attr))?;
                            let field_type = si.fields[field_idx].1.clone();

                            self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                            self.compile_expr(&assign.value, None, body, locals, names, gs)?;
                            let vt = self.infer_type(&assign.value, names);
                            self.emit_coercion(body, vt, field_type);
                            body.push(Instruction::StructSet {
                                struct_type_index: si.type_idx,
                                field_index: field_idx as u32,
                            });
                            return Ok(());
                        } else {
                            return Err("Attributes only supported on structs".to_string());
                        }
                    }
                }
                Err("Unsupported assignment".to_string())
            }
            Stmt::AugAssign(assign) => {
                if let Expr::Name(name) = assign.target.as_ref() {
                    let (wt, getter, setter) = if let Some(gi) = self.find_global(&name.id) {
                        if gs.contains(&name.id.to_string())
                            || !names.iter().any(|(n, _)| n == name.id.as_str())
                        {
                            (
                                gi.wasm_type.clone(),
                                Instruction::GlobalGet(gi.index),
                                Instruction::GlobalSet(gi.index),
                            )
                        } else {
                            let li = names
                                .iter()
                                .position(|(n, _)| n == name.id.as_str())
                                .ok_or_else(|| format!("Undefined variable: {}", name.id))?;
                            (
                                names[li].1.clone(),
                                Instruction::LocalGet(li as u32),
                                Instruction::LocalSet(li as u32),
                            )
                        }
                    } else {
                        let li = names
                            .iter()
                            .position(|(n, _)| n == name.id.as_str())
                            .ok_or_else(|| format!("Undefined variable: {}", name.id))?;
                        (
                            names[li].1.clone(),
                            Instruction::LocalGet(li as u32),
                            Instruction::LocalSet(li as u32),
                        )
                    };

                    body.push(getter);
                    self.compile_expr(&assign.value, None, body, locals, names, gs)?;
                    let vt = self.infer_type(&assign.value, names);
                    self.emit_coercion(body, vt, wt.clone());
                    self.emit_binop(assign.op, wt, body)?;
                    body.push(setter);
                    Ok(())
                } else if let Expr::Attribute(attr) = assign.target.as_ref() {
                    let obj_type = self.infer_type(&attr.value, names);
                    if let WasmType::Struct(struct_name) = obj_type {
                        let si = self.struct_types.get(&struct_name).cloned().ok_or_else(|| {
                            format!("Unknown struct: {}", struct_name)
                        })?;
                        let field_idx = si
                            .fields
                            .iter()
                            .position(|(n, _)| n == attr.attr.as_str())
                            .ok_or_else(|| format!("Unknown field: {}", attr.attr))?;
                        let field_type = si.fields[field_idx].1.clone();

                        self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                        let tmp = self.alloc_temp(locals, names, WasmType::Struct(struct_name));
                        body.push(Instruction::LocalTee(tmp));
                        body.push(Instruction::LocalGet(tmp));
                        body.push(Instruction::StructGet {
                            struct_type_index: si.type_idx,
                            field_index: field_idx as u32,
                        });
                        self.compile_expr(&assign.value, None, body, locals, names, gs)?;
                        let vt = self.infer_type(&assign.value, names);
                        self.emit_coercion(body, vt, field_type.clone());
                        self.emit_binop(assign.op, field_type, body)?;
                        body.push(Instruction::StructSet {
                            struct_type_index: si.type_idx,
                            field_index: field_idx as u32,
                        });
                        Ok(())
                    } else {
                        Err("Attributes only supported on structs".to_string())
                    }
                } else {
                    Err("Unsupported augmented assignment target".to_string())
                }
            }
            Stmt::While(w) => {
                body.push(Instruction::Block(wasm_encoder::BlockType::Empty));
                body.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
                self.compile_expr(&w.test, None, body, locals, names, gs)?;
                let vt = self.infer_type(&w.test, names);
                if vt == WasmType::F64 {
                    body.push(Instruction::F64Const(0.0f64.into()));
                    body.push(Instruction::F64Ne);
                } else {
                    body.push(Instruction::I64Const(0));
                    body.push(Instruction::I64Ne);
                }
                body.push(Instruction::I32Eqz);
                body.push(Instruction::BrIf(1));

                for s in &w.body {
                    self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                }
                body.push(Instruction::Br(0));
                body.push(Instruction::End);
                body.push(Instruction::End);

                for s in &w.orelse {
                    self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                }
                Ok(())
            }
            Stmt::For(f) => {
                let iter_type = self.infer_type(&f.iter, names);
                let elem_type = if let WasmType::List(et) = iter_type {
                    et.as_ref().clone()
                } else {
                    return Err("Can only iterate over lists".to_string());
                };
                let array_idx = self.array_types.get(&elem_type).copied().ok_or_else(|| {
                    format!("Array type not registered for {:?}", elem_type)
                })?;
                let list_idx = self.list_types.get(&elem_type).copied().unwrap();

                // Temp locals
                let list_tmp = self.alloc_temp(locals, names, WasmType::List(Box::new(elem_type.clone())));
                let len_tmp = self.alloc_temp(locals, names, WasmType::I64); // actually i32 but we only have i64
                let i_tmp = self.alloc_temp(locals, names, WasmType::I64);

                // Initialize
                self.compile_expr(&f.iter, None, body, locals, names, gs)?;
                body.push(Instruction::LocalTee(list_tmp));
                body.push(Instruction::StructGet {
                    struct_type_index: list_idx,
                    field_index: 1,
                });
                body.push(Instruction::I64ExtendI32U);
                body.push(Instruction::LocalSet(len_tmp));
                body.push(Instruction::I64Const(0));
                body.push(Instruction::LocalSet(i_tmp));

                body.push(Instruction::Block(wasm_encoder::BlockType::Empty));
                body.push(Instruction::Loop(wasm_encoder::BlockType::Empty));
                
                // Condition: i < len
                body.push(Instruction::LocalGet(i_tmp));
                body.push(Instruction::LocalGet(len_tmp));
                body.push(Instruction::I64GeU);
                body.push(Instruction::BrIf(1));

                // Target assignment
                if let Expr::Name(name) = f.target.as_ref() {
                    let li = if let Some(pos) = names.iter().position(|(n, _)| n == name.id.as_str()) {
                        pos as u32
                    } else {
                        let li = names.len() as u32;
                        locals.push((1, self.wasm_type_to_val_type(&elem_type)));
                        names.push((name.id.to_string(), elem_type.clone()));
                        li
                    };
                    body.push(Instruction::LocalGet(list_tmp));
                    body.push(Instruction::StructGet {
                        struct_type_index: list_idx,
                        field_index: 0,
                    });
                    body.push(Instruction::LocalGet(i_tmp));
                    body.push(Instruction::I32WrapI64);
                    body.push(Instruction::ArrayGet(array_idx));
                    body.push(Instruction::LocalSet(li));
                } else {
                    return Err("Unsupported for loop target".to_string());
                }

                // Body
                for s in &f.body {
                    self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                }

                // Increment
                body.push(Instruction::LocalGet(i_tmp));
                body.push(Instruction::I64Const(1));
                body.push(Instruction::I64Add);
                body.push(Instruction::LocalSet(i_tmp));
                body.push(Instruction::Br(0));

                body.push(Instruction::End);
                body.push(Instruction::End);
                
                for s in &f.orelse {
                    self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                }
                Ok(())
            }
            Stmt::Assert(a) => {
                self.compile_expr(&a.test, None, body, locals, names, gs)?;
                body.push(Instruction::I64Eqz);
                body.push(Instruction::If(wasm_encoder::BlockType::Empty));
                body.push(Instruction::Unreachable);
                body.push(Instruction::End);
                Ok(())
            }
            Stmt::ClassDef(_) | Stmt::ImportFrom(_) | Stmt::Pass(_) => Ok(()),
            Stmt::If(if_stmt) => {
                self.compile_expr(&if_stmt.test, None, body, locals, names, gs)?;
                let vt = self.infer_type(&if_stmt.test, names);
                if vt == WasmType::F64 {
                    body.push(Instruction::F64Const(0.0f64.into()));
                    body.push(Instruction::F64Ne);
                } else {
                    body.push(Instruction::I64Const(0));
                    body.push(Instruction::I64Ne);
                }
                body.push(Instruction::If(wasm_encoder::BlockType::Empty));
                for s in &if_stmt.body {
                    self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                }

                if !if_stmt.elif_else_clauses.is_empty() {
                    body.push(Instruction::Else);
                    self.compile_if_clauses(&if_stmt.elif_else_clauses, ret_type, body, locals, names, gs)?;
                }


                body.push(Instruction::End);
                Ok(())
            }
            Stmt::Match(m) => {
                let subject_type = self.infer_type(&m.subject, names);
                let subject_tmp = self.alloc_temp(locals, names, subject_type.clone());
                self.compile_expr(&m.subject, None, body, locals, names, gs)?;
                body.push(Instruction::LocalSet(subject_tmp));

                body.push(Instruction::Block(wasm_encoder::BlockType::Empty)); // End of match (index N)

                for _ in &m.cases {
                    body.push(Instruction::Block(wasm_encoder::BlockType::Empty)); // Skip to next case
                }

                let n = m.cases.len() as u32;
                for (i, case) in m.cases.iter().enumerate() {
                    // Pattern matching
                    match &case.pattern {
                        ruff_python_ast::Pattern::MatchValue(v) => {
                            body.push(Instruction::LocalGet(subject_tmp));
                            let subject_val_type = if let WasmType::Struct(ref name) = subject_type {
                                if self.enum_types.contains_key(name) {
                                    self.emit_coercion(body, subject_type.clone(), WasmType::I64);
                                    WasmType::I64
                                } else {
                                    subject_type.clone()
                                }
                            } else {
                                subject_type.clone()
                            };
                            self.compile_expr(&v.value, None, body, locals, names, gs)?;
                            let v_type = self.infer_type(&v.value, names);
                            if subject_val_type == WasmType::I64 && v_type != WasmType::I64 {
                                self.emit_coercion(body, v_type, WasmType::I64);
                            }
                            self.emit_cmp(body, ruff_python_ast::CmpOp::Eq, subject_val_type)?;
                            body.push(Instruction::I32Eqz);
                            body.push(Instruction::BrIf(0)); // Skip to next block if not equal
                        }
                        ruff_python_ast::Pattern::MatchAs(as_pat) => {
                            if as_pat.name.is_some() {
                                return Err("Named match patterns not supported yet".to_string());
                            }
                            // Wildcard _ : always matches, nothing to do
                        }
                        _ => return Err("Unsupported match pattern".to_string()),
                    }

                    // Body
                    for s in &case.body {
                        self.compile_stmt(s, ret_type, body, locals, names, gs)?;
                    }
                    body.push(Instruction::Br(n - i as u32)); // Jump to end of match
                    body.push(Instruction::End); // End of case block
                }

                body.push(Instruction::End); // End of match block
                Ok(())
            }
            Stmt::Expr(e) => {
                self.compile_expr(&e.value, None, body, locals, names, gs)?;
                let vt = self.infer_type(&e.value, names);
                if vt != WasmType::None {
                    body.push(Instruction::Drop);
                }
                Ok(())
            }
            _ => Err("Unsupported statement".to_string()),
        }
    }

    fn compile_if_clauses<'a>(
        &mut self,
        clauses: &[ruff_python_ast::ElifElseClause],
        ret_type: &WasmType,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        let Some((clause, rest)) = clauses.split_first() else {
            return Ok(());
        };

        if let Some(test) = &clause.test {
            // This is an 'elif'
            self.compile_expr(test, None, body, locals, names, gs)?;
            let vt = self.infer_type(test, names);
            if vt == WasmType::F64 {
                body.push(Instruction::F64Const(0.0f64.into()));
                body.push(Instruction::F64Ne);
            } else {
                body.push(Instruction::I64Const(0));
                body.push(Instruction::I64Ne);
            }
            body.push(Instruction::If(wasm_encoder::BlockType::Empty));
            for s in &clause.body {
                self.compile_stmt(s, ret_type, body, locals, names, gs)?;
            }
            if !rest.is_empty() {
                body.push(Instruction::Else);
                self.compile_if_clauses(rest, ret_type, body, locals, names, gs)?;
            }
            body.push(Instruction::End);
        } else {
            // This is an 'else'
            for s in &clause.body {
                self.compile_stmt(s, ret_type, body, locals, names, gs)?;
            }
        }
        Ok(())
    }

    fn compile_expr<'a>(
        &mut self,
        expr: &Expr,
        expected_type: Option<&WasmType>,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        match expr {
            Expr::NumberLiteral(lit) => match &lit.value {
                ruff_python_ast::Number::Int(n) => {
                    body.push(Instruction::I64Const(
                        n.as_i64().ok_or("Integer too large")?,
                    ));
                    Ok(())
                }
                ruff_python_ast::Number::Float(f) => {
                    body.push(Instruction::F64Const((*f).into()));
                    Ok(())
                }
                _ => Err("Unsupported number type".to_string()),
            },
            Expr::StringLiteral(lit) => {
                let array_idx = self.array_types.get(&WasmType::I64).copied().ok_or_else(|| {
                    "Array type for strings (I64) not registered".to_string()
                })?;
                let s = lit.value.to_str();
                for c in s.chars() {
                    body.push(Instruction::I64Const(c as i64));
                }
                body.push(Instruction::ArrayNewFixed {
                    array_type_index: array_idx,
                    array_size: s.chars().count() as u32,
                });
                Ok(())
            }
            Expr::NoneLiteral(_) => {
                body.push(Instruction::RefNull(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::None,
                }));
                Ok(())
            }
            Expr::BooleanLiteral(lit) => {
                body.push(Instruction::I64Const(if lit.value { 1 } else { 0 }));
                Ok(())
            }
            Expr::Name(name) => {
                if let Some(gi) = self.find_global(&name.id) {
                    if gs.contains(&name.id.to_string())
                        || !names.iter().any(|(n, _)| n == name.id.as_str())
                    {
                        body.push(Instruction::GlobalGet(gi.index));
                        return Ok(());
                    }
                }
                let i = names
                    .iter()
                    .position(|(n, _)| n == name.id.as_str())
                    .ok_or_else(|| {
                        if let Some(fi) = self.func_infos.iter().position(|f| f.name == name.id.as_str()) {
                            return (fi as u32, true);
                        }
                        (0, false)
                    });

                match i {
                    Ok(idx) => {
                        body.push(Instruction::LocalGet(idx as u32));
                        Ok(())
                    }
                    Err((idx, true)) => {
                        body.push(Instruction::RefFunc(idx));
                        Ok(())
                    }
                    Err(_) => Err(format!("Undefined variable: {}", name.id)),
                }
            }
            Expr::BinOp(binop) => {
                let lt = self.infer_type(&binop.left, names);
                let rt = self.infer_type(&binop.right, names);
                let result = if binop.op == ruff_python_ast::Operator::Div {
                    WasmType::F64
                } else {
                    self.promote(lt.clone(), rt.clone())
                };

                if binop.op == ruff_python_ast::Operator::Mod && result == WasmType::F64 {
                    return self.compile_float_mod(
                        &binop.left,
                        &binop.right,
                        lt,
                        rt,
                        body,
                        locals,
                        names,
                        gs,
                    );
                }

                self.compile_expr(&binop.left, None, body, locals, names, gs)?;
                self.emit_coercion(body, lt, result.clone());
                self.compile_expr(&binop.right, None, body, locals, names, gs)?;
                self.emit_coercion(body, rt, result.clone());

                self.emit_binop(binop.op, result, body)?;
                Ok(())
            }
            Expr::UnaryOp(u) => match u.op {
                ruff_python_ast::UnaryOp::Not => {
                    self.compile_expr(&u.operand, None, body, locals, names, gs)?;
                    body.push(Instruction::I64Eqz);
                    body.push(Instruction::I64ExtendI32U);
                    Ok(())
                }
                ruff_python_ast::UnaryOp::USub => {
                    if self.infer_type(&u.operand, names) == WasmType::F64 {
                        self.compile_expr(&u.operand, None, body, locals, names, gs)?;
                        body.push(Instruction::F64Neg);
                    } else {
                        body.push(Instruction::I64Const(0));
                        self.compile_expr(&u.operand, None, body, locals, names, gs)?;
                        body.push(Instruction::I64Sub);
                    }
                    Ok(())
                }
                _ => Err("Unsupported unary operator".to_string()),
            },
            Expr::BoolOp(b) => {
                self.compile_expr(&b.values[0], None, body, locals, names, gs)?;
                for v in &b.values[1..] {
                    self.compile_expr(v, None, body, locals, names, gs)?;
                    match b.op {
                        ruff_python_ast::BoolOp::And => body.push(Instruction::I64And),
                        ruff_python_ast::BoolOp::Or => body.push(Instruction::I64Or),
                    }
                }
                Ok(())
            }
            Expr::Compare(cmp) => {
                if cmp.ops.len() == 1 {
                    self.compile_single_cmp(
                        &cmp.left,
                        cmp.ops[0],
                        &cmp.comparators[0],
                        body,
                        locals,
                        names,
                        gs,
                    )
                } else {
                    self.compile_chained_cmp(cmp, body, locals, names, gs)
                }
            }
            Expr::Call(call) => {
                if let Expr::Attribute(attr) = call.func.as_ref() {
                    let obj_type = self.infer_type(&attr.value, names);
                    if let WasmType::List(ref et) = obj_type {
                        if attr.attr.as_str() == "append" && call.arguments.args.len() == 1 {
                            let array_idx = self.array_types.get(et).copied().unwrap();
                            let list_idx = self.list_types.get(et).copied().unwrap();

                            // Temps
                            let list_tmp = self.alloc_temp(locals, names, WasmType::List(et.clone()));
                            let old_array_tmp = self.alloc_temp(locals, names, WasmType::Array(et.clone()));
                            let len_tmp = self.alloc_temp(locals, names, WasmType::I32); // used as i32 for array ops

                            // list = obj
                            self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                            body.push(Instruction::LocalTee(list_tmp));
                            // len = list.len
                            body.push(Instruction::StructGet { struct_type_index: list_idx, field_index: 1 });
                            body.push(Instruction::LocalSet(len_tmp));
                            
                            // old_array = list.data
                            body.push(Instruction::LocalGet(list_tmp));
                            body.push(Instruction::StructGet { struct_type_index: list_idx, field_index: 0 });
                            body.push(Instruction::LocalSet(old_array_tmp));

                            // new_array = array.new(default, len + 1)
                            self.emit_default_value(et, body)?;
                            body.push(Instruction::LocalGet(len_tmp));
                            body.push(Instruction::I32Const(1));
                            body.push(Instruction::I32Add);
                            body.push(Instruction::ArrayNew(array_idx));
                            
                            let new_array_tmp = self.alloc_temp(locals, names, WasmType::Array(et.clone()));
                            body.push(Instruction::LocalSet(new_array_tmp));
                            
                            // array.copy(new_array, 0, old_array, 0, len)
                            body.push(Instruction::LocalGet(new_array_tmp));
                            body.push(Instruction::I32Const(0)); // dest offset
                            body.push(Instruction::LocalGet(old_array_tmp));
                            body.push(Instruction::I32Const(0)); // src offset
                            body.push(Instruction::LocalGet(len_tmp));
                            body.push(Instruction::ArrayCopy {
                                array_type_index_dst: array_idx,
                                array_type_index_src: array_idx,
                            });
                            
                            // new_array[len] = val
                            body.push(Instruction::LocalGet(new_array_tmp));
                            body.push(Instruction::LocalGet(len_tmp));
                            self.compile_expr(&call.arguments.args[0], None, body, locals, names, gs)?;
                            body.push(Instruction::ArraySet(array_idx));
                            
                            // list.data = new_array
                            body.push(Instruction::LocalGet(list_tmp));
                            body.push(Instruction::LocalGet(new_array_tmp));
                            body.push(Instruction::StructSet { struct_type_index: list_idx, field_index: 0 });
                            
                            // list.len = len + 1
                            body.push(Instruction::LocalGet(list_tmp));
                            body.push(Instruction::LocalGet(len_tmp));
                            body.push(Instruction::I32Const(1));
                            body.push(Instruction::I32Add);
                            body.push(Instruction::StructSet { struct_type_index: list_idx, field_index: 1 });
                            return Ok(());
                        }
                    }

                    if let WasmType::Struct(class_name) = obj_type {
                        if let Some(si) = self.struct_types.get(&class_name).cloned() {
                            if let Some(&func_idx) = si.methods.get(attr.attr.as_str()) {
                                // Push 'self'
                                self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                                // Push other arguments
                                for (i, arg) in call.arguments.args.iter().enumerate() {
                                    let expected = self.func_infos[func_idx as usize].param_types.get(i + 1).cloned(); // +1 because self is arg 0
                                    self.compile_expr(arg, expected.as_ref(), body, locals, names, gs)?;
                                    if let Some(et) = expected {
                                        self.emit_coercion(body, self.infer_type(arg, names), et.clone());
                                    }
                                }
                                body.push(Instruction::Call(func_idx));
                                return Ok(());
                            }
                        }
                    }
                }

                if let Expr::Name(fname) = call.func.as_ref() {
                    if fname.id == "len" && call.arguments.args.len() == 1 {
                        let arg_type = self.infer_type(&call.arguments.args[0], names);
                        self.compile_expr(&call.arguments.args[0], None, body, locals, names, gs)?;
                        if let WasmType::List(et) = arg_type {
                            let list_idx = self.list_types.get(&et).copied().unwrap();
                            body.push(Instruction::StructGet {
                                struct_type_index: list_idx,
                                field_index: 1,
                            });
                        } else {
                            body.push(Instruction::ArrayLen);
                        }
                        body.push(Instruction::I64ExtendI32U);
                        return Ok(());
                    }
                    if fname.id == "auto" {
                        body.push(Instruction::I64Const(0));
                        return Ok(());
                    }
                    if let Some(si) = self.struct_types.get(fname.id.as_str()).cloned() {
                        if let Some(&init_idx) = si.methods.get("__init__") {
                            for (_, wt) in &si.fields {
                                self.emit_default_value(wt, body)?;
                            }
                            body.push(Instruction::StructNew(si.type_idx));
                            let tmp = self.alloc_temp(locals, names, WasmType::Struct(fname.id.to_string()));
                            body.push(Instruction::LocalTee(tmp));
                            for (i, arg) in call.arguments.args.iter().enumerate() {
                                let expected = self.func_infos[init_idx as usize].param_types.get(i + 1).cloned(); // +1 for self
                                self.compile_expr(arg, expected.as_ref(), body, locals, names, gs)?;
                                if let Some(et) = expected {
                                    self.emit_coercion(body, self.infer_type(arg, names), et.clone());
                                }
                            }
                            body.push(Instruction::Call(init_idx));
                            body.push(Instruction::Drop);
                            body.push(Instruction::LocalGet(tmp));
                        } else {
                            for (i, (_, wt)) in si.fields.iter().enumerate() {
                                if i < call.arguments.args.len() {
                                    self.compile_expr(&call.arguments.args[i], None, body, locals, names, gs)?;
                                } else {
                                    self.emit_default_value(wt, body)?;
                                }
                            }
                            body.push(Instruction::StructNew(si.type_idx));
                        }
                        return Ok(());
                    }
                    if let Some(fi) = self
                        .func_infos
                        .iter()
                        .position(|f| f.name == fname.id.as_str())
                    {
                        for (i, arg) in call.arguments.args.iter().enumerate() {
                            let expected = self.func_infos[fi].param_types.get(i).cloned();
                            self.compile_expr(arg, expected.as_ref(), body, locals, names, gs)?;
                            if let Some(et) = expected {
                                self.emit_coercion(body, self.infer_type(arg, names), et.clone());
                            }
                        }
                        body.push(Instruction::Call(fi as u32));
                        return Ok(());
                    }
                }

                let func_type = self.infer_type(&call.func, names);
                if let WasmType::Callable { params, ret } = func_type {
                    for (i, arg) in call.arguments.args.iter().enumerate() {
                        let expected = params.get(i);
                        self.compile_expr(arg, expected, body, locals, names, gs)?;
                        if let Some(et) = expected {
                            self.emit_coercion(body, self.infer_type(arg, names), et.clone());
                        }
                    }
                    self.compile_expr(&call.func, None, body, locals, names, gs)?;

                    let wasm_params: Vec<ValType> = params
                        .iter()
                        .map(|t| self.wasm_type_to_val_type(t))
                        .collect();
                    let wasm_ret = self.wasm_type_to_val_type(&ret);
                    let type_idx = self.type_count;
                    self.types.ty().function(wasm_params, vec![wasm_ret]);
                    self.type_count += 1;

                    body.push(Instruction::CallRef(type_idx));
                    Ok(())
                } else {
                    Err("Unsupported call target".to_string())
                }
            }
            Expr::Attribute(attr) => {
                if let Expr::Name(name) = attr.value.as_ref() {
                    if self.enum_types.contains_key(name.id.as_str()) {
                        let full_name = format!("{}.{}", name.id, attr.attr);
                        if let Some(gi) = self.find_global(&full_name) {
                            body.push(Instruction::GlobalGet(gi.index));
                            return Ok(());
                        }
                    }
                }

                let val_type = self.infer_type(&attr.value, names);
                match val_type {
                    WasmType::Struct(ref struct_name) => {
                        if let Some(ei) = self.enum_types.get(struct_name).cloned() {
                            // Enum instance access: .name or .value
                            self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                            if attr.attr.as_str() == "value" {
                                body.push(Instruction::StructGet {
                                    struct_type_index: ei.type_idx,
                                    field_index: 0,
                                });
                            } else if attr.attr.as_str() == "name" {
                                body.push(Instruction::StructGet {
                                    struct_type_index: ei.type_idx,
                                    field_index: 1,
                                });
                            } else {
                                return Err(format!("Unknown enum attribute: {}", attr.attr));
                            }
                            Ok(())
                        } else if let Some(si) = self.struct_types.get(struct_name).cloned() {
                            let field_idx = si
                                .fields
                                .iter()
                                .position(|(n, _)| n == attr.attr.as_str())
                                .ok_or_else(|| format!("Unknown field: {}", attr.attr))?;

                            self.compile_expr(&attr.value, None, body, locals, names, gs)?;
                            body.push(Instruction::StructGet {
                                struct_type_index: si.type_idx,
                                field_index: field_idx as u32,
                            });
                            Ok(())
                        } else {
                            Err(format!("Unknown struct: {}", struct_name))
                        }
                    }
                    _ => Err("Attributes only supported on structs and enums".to_string()),
                }
            }
            Expr::List(list) => {
                let elem_type = if let Some(WasmType::List(et)) = expected_type {
                    et.as_ref().clone()
                } else if !list.elts.is_empty() {
                    self.infer_type(&list.elts[0], names)
                } else {
                    WasmType::I64
                };
                let array_idx = self.array_types.get(&elem_type).copied().ok_or_else(|| {
                    format!("Array type not registered for {:?}", elem_type)
                })?;
                let list_idx = self.list_types.get(&elem_type).copied().ok_or_else(|| {
                    format!("List type not registered for {:?}", elem_type)
                })?;

                for elt in &list.elts {
                    self.compile_expr(elt, None, body, locals, names, gs)?;
                    self.emit_coercion(body, self.infer_type(elt, names), elem_type.clone());
                }
                body.push(Instruction::ArrayNewFixed {
                    array_type_index: array_idx,
                    array_size: list.elts.len() as u32,
                });
                body.push(Instruction::I32Const(list.elts.len() as i32));
                body.push(Instruction::StructNew(list_idx));
                Ok(())
            }
            Expr::Subscript(sub) => {
                let vt = self.infer_type(&sub.value, names);
                let (array_idx, list_idx) = match &vt {
                    WasmType::List(et) => (
                        self.array_types.get(et).copied().ok_or_else(|| {
                            format!("Array type not registered for {:?}", et)
                        })?,
                        self.list_types.get(et).copied(),
                    ),
                    WasmType::String => (
                        self.array_types.get(&WasmType::I64).copied().ok_or("Array type for I64 not registered")?,
                        None,
                    ),
                    _ => return Err(format!("Indexing not supported for {:?}", vt)),
                };

                self.compile_expr(&sub.value, None, body, locals, names, gs)?;
                if let Some(l_idx) = list_idx {
                    body.push(Instruction::StructGet {
                        struct_type_index: l_idx,
                        field_index: 0,
                    });
                }
                self.compile_expr(&sub.slice, None, body, locals, names, gs)?;
                body.push(Instruction::I32WrapI64);
                body.push(Instruction::ArrayGet(array_idx));
                Ok(())
            }
            Expr::Lambda(lambda) => {
                self.lambda_count += 1;
                let name = format!("__lambda_{}", self.lambda_count);

                let mut param_names = Vec::new();
                let mut param_types = Vec::new();
                if let Some(params) = &lambda.parameters {
                    for p in params.posonlyargs.iter().chain(params.args.iter()) {
                        param_names.push(p.parameter.name.to_string());
                        param_types.push(WasmType::I64); // Default to I64 for lambda params
                    }
                }

                let mut lambda_names = Vec::new();
                for (i, p_name) in param_names.iter().enumerate() {
                    lambda_names.push((p_name.clone(), param_types[i].clone()));
                }
                let ret_type = self.infer_type(&lambda.body, &lambda_names);

                let func_idx = self.compile_func_low_level(
                    name,
                    param_names,
                    param_types,
                    ret_type,
                    Some(&lambda.body),
                    None,
                )?;

                body.push(Instruction::RefFunc(func_idx));
                Ok(())
            }
            Expr::Dict(dict) => {
                let (kt, vt) = if let Some(WasmType::Dict(kt, vt)) = expected_type {
                    (kt.as_ref().clone(), vt.as_ref().clone())
                } else if !dict.items.is_empty() {
                    let k = self.infer_type(&dict.items[0].key.as_ref().unwrap(), names);
                    let v = self.infer_type(&dict.items[0].value, names);
                    (k, v)
                } else {
                    (WasmType::I64, WasmType::I64)
                };
                let kt_idx = self.array_types.get(&kt).copied().unwrap();
                let vt_idx = self.array_types.get(&vt).copied().unwrap();
                let dict_idx = self.dict_types.get(&(kt.clone(), vt.clone())).copied().unwrap();

                for item in &dict.items {
                    self.compile_expr(item.key.as_ref().unwrap(), Some(&kt), body, locals, names, gs)?;
                }
                body.push(Instruction::ArrayNewFixed {
                    array_type_index: kt_idx,
                    array_size: dict.items.len() as u32,
                });
                for item in &dict.items {
                    self.compile_expr(&item.value, Some(&vt), body, locals, names, gs)?;
                }
                body.push(Instruction::ArrayNewFixed {
                    array_type_index: vt_idx,
                    array_size: dict.items.len() as u32,
                });
                body.push(Instruction::I32Const(dict.items.len() as i32));
                body.push(Instruction::StructNew(dict_idx));
                Ok(())
            }
            Expr::Set(set) => {
                let et = if let Some(WasmType::Set(et)) = expected_type {
                    et.as_ref().clone()
                } else if !set.elts.is_empty() {
                    self.infer_type(&set.elts[0], names)
                } else {
                    WasmType::I64
                };
                let et_idx = self.array_types.get(&et).copied().unwrap();
                let set_idx = self.set_types.get(&et).copied().unwrap();

                for elt in &set.elts {
                    self.compile_expr(elt, Some(&et), body, locals, names, gs)?;
                }
                body.push(Instruction::ArrayNewFixed {
                    array_type_index: et_idx,
                    array_size: set.elts.len() as u32,
                });
                body.push(Instruction::I32Const(set.elts.len() as i32));
                body.push(Instruction::StructNew(set_idx));
                Ok(())
            }
            Expr::ListComp(lc) => {
                let elem_type = self.infer_type(&lc.elt, names);
                let list_idx = self.list_types.get(&elem_type).copied().ok_or_else(|| {
                    format!("List type not registered for {:?}", elem_type)
                })?;
                let array_idx = self.array_types.get(&elem_type).copied().unwrap();

                // 1. Initialize empty list
                body.push(Instruction::I32Const(0));
                self.emit_default_value(&elem_type, body)?;
                body.push(Instruction::ArrayNew(array_idx));
                body.push(Instruction::I32Const(0));
                body.push(Instruction::StructNew(list_idx));
                
                let list_tmp = self.alloc_temp(locals, names, WasmType::List(Box::new(elem_type.clone())));
                body.push(Instruction::LocalSet(list_tmp));

                // 2. Loop through generators
                // For simplicity, we only support one generator for now
                if lc.generators.len() != 1 {
                    return Err("Only single generator comprehensions supported".to_string());
                }
                let generator = &lc.generators[0];
                
                // This is essentially a for loop
                let iter_type = self.infer_type(&generator.iter, names);
                let gen_elem_type = if let WasmType::List(et) = iter_type {
                    et.as_ref().clone()
                } else {
                    return Err("Can only iterate over lists in comprehension".to_string());
                };
                let gen_array_idx = self.array_types.get(&gen_elem_type).copied().unwrap();
                let gen_list_idx = self.list_types.get(&gen_elem_type).copied().unwrap();

                let iter_list_tmp = self.alloc_temp(locals, names, WasmType::List(Box::new(gen_elem_type.clone())));
                let len_tmp = self.alloc_temp(locals, names, WasmType::I64);
                let i_tmp = self.alloc_temp(locals, names, WasmType::I64);

                self.compile_expr(&generator.iter, None, body, locals, names, gs)?;
                body.push(Instruction::LocalTee(iter_list_tmp));
                body.push(Instruction::StructGet { struct_type_index: gen_list_idx, field_index: 1 });
                body.push(Instruction::I64ExtendI32U);
                body.push(Instruction::LocalSet(len_tmp));
                body.push(Instruction::I64Const(0));
                body.push(Instruction::LocalSet(i_tmp));

                body.push(Instruction::Block(BlockType::Empty));
                body.push(Instruction::Loop(BlockType::Empty));
                body.push(Instruction::LocalGet(i_tmp));
                body.push(Instruction::LocalGet(len_tmp));
                body.push(Instruction::I64GeU);
                body.push(Instruction::BrIf(1));

                // Set target
                if let Expr::Name(ref name) = generator.target {
                    let li = if let Some(pos) = names.iter().position(|(n, _)| n == name.id.as_str()) {
                        pos as u32
                    } else {
                        let li = names.len() as u32;
                        locals.push((1, self.wasm_type_to_val_type(&gen_elem_type)));
                        names.push((name.id.to_string(), gen_elem_type.clone()));
                        li
                    };
                    body.push(Instruction::LocalGet(iter_list_tmp));
                    body.push(Instruction::StructGet { struct_type_index: gen_list_idx, field_index: 0 });
                    body.push(Instruction::LocalGet(i_tmp));
                    body.push(Instruction::I32WrapI64);
                    body.push(Instruction::ArrayGet(gen_array_idx));
                    body.push(Instruction::LocalSet(li));
                }

                // If conditions
                for if_expr in &generator.ifs {
                    self.compile_expr(if_expr, None, body, locals, names, gs)?;
                    body.push(Instruction::I64Eqz);
                    body.push(Instruction::BrIf(0)); // Skip to next iteration
                }

                // Append elt to list_tmp
                // list = list_tmp
                body.push(Instruction::LocalGet(list_tmp));
                // value = compile(lc.elt)
                self.compile_expr(&lc.elt, None, body, locals, names, gs)?;
                
                // Manual inline of append logic (simplified for this context)
                let val_tmp = self.alloc_temp(locals, names, elem_type.clone());
                body.push(Instruction::LocalSet(val_tmp));
                let l_tmp = self.alloc_temp(locals, names, WasmType::List(Box::new(elem_type.clone())));
                body.push(Instruction::LocalSet(l_tmp));
                
                let cur_len_tmp = self.alloc_temp(locals, names, WasmType::I32);
                body.push(Instruction::LocalGet(l_tmp));
                body.push(Instruction::StructGet { struct_type_index: list_idx, field_index: 1 });
                body.push(Instruction::LocalSet(cur_len_tmp));

                let old_arr_tmp = self.alloc_temp(locals, names, WasmType::Array(Box::new(elem_type.clone())));
                body.push(Instruction::LocalGet(l_tmp));
                body.push(Instruction::StructGet { struct_type_index: list_idx, field_index: 0 });
                body.push(Instruction::LocalSet(old_arr_tmp));

                self.emit_default_value(&elem_type, body)?;
                body.push(Instruction::LocalGet(cur_len_tmp));
                body.push(Instruction::I32Const(1));
                body.push(Instruction::I32Add);
                body.push(Instruction::ArrayNew(array_idx));
                let new_arr_tmp = self.alloc_temp(locals, names, WasmType::Array(Box::new(elem_type.clone())));
                body.push(Instruction::LocalTee(new_arr_tmp));
                body.push(Instruction::I32Const(0));
                body.push(Instruction::LocalGet(old_arr_tmp));
                body.push(Instruction::I32Const(0));
                body.push(Instruction::LocalGet(cur_len_tmp));
                body.push(Instruction::ArrayCopy { array_type_index_dst: array_idx, array_type_index_src: array_idx });
                
                body.push(Instruction::LocalGet(new_arr_tmp));
                body.push(Instruction::LocalGet(cur_len_tmp));
                body.push(Instruction::LocalGet(val_tmp));
                body.push(Instruction::ArraySet(array_idx));

                body.push(Instruction::LocalGet(l_tmp));
                body.push(Instruction::LocalGet(new_arr_tmp));
                body.push(Instruction::StructSet { struct_type_index: list_idx, field_index: 0 });
                body.push(Instruction::LocalGet(l_tmp));
                body.push(Instruction::LocalGet(cur_len_tmp));
                body.push(Instruction::I32Const(1));
                body.push(Instruction::I32Add);
                body.push(Instruction::StructSet { struct_type_index: list_idx, field_index: 1 });

                // End loop
                body.push(Instruction::LocalGet(i_tmp));
                body.push(Instruction::I64Const(1));
                body.push(Instruction::I64Add);
                body.push(Instruction::LocalSet(i_tmp));
                body.push(Instruction::Br(0));
                body.push(Instruction::End);
                body.push(Instruction::End);

                body.push(Instruction::LocalGet(list_tmp));
                Ok(())
            }
            _ => Err("Unsupported expression".to_string()),
        }
    }

    fn compile_single_cmp<'a>(
        &mut self,
        left: &Expr,
        op: ruff_python_ast::CmpOp,
        right: &Expr,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        let lt = self.infer_type(left, names);
        let rt = self.infer_type(right, names);
        let ct = self.promote(lt.clone(), rt.clone());
        self.compile_expr(left, Some(&ct), body, locals, names, gs)?;
        self.emit_coercion(body, lt.clone(), ct.clone());
        self.compile_expr(right, Some(&ct), body, locals, names, gs)?;
        self.emit_coercion(body, rt.clone(), ct.clone());

        if let WasmType::Struct(ref name) = ct {
            if let Some(ei) = self.enum_types.get(name).cloned() {
                // It's an enum, we need to compare .value
                // Current stack: [..., enum_a, enum_b]
                let tb = self.alloc_temp(locals, names, ct.clone());
                body.push(Instruction::LocalSet(tb));
                body.push(Instruction::StructGet {
                    struct_type_index: ei.type_idx,
                    field_index: 0,
                });
                body.push(Instruction::LocalGet(tb));
                body.push(Instruction::StructGet {
                    struct_type_index: ei.type_idx,
                    field_index: 0,
                });
                self.emit_cmp(body, op, WasmType::I64)?;
            } else {
                self.emit_cmp(body, op, ct)?;
            }
        } else {
            self.emit_cmp(body, op, ct)?;
        }
        body.push(Instruction::I64ExtendI32U);
        Ok(())
    }

    fn compile_chained_cmp<'a>(
        &mut self,
        cmp: &ruff_python_ast::ExprCompare,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut prev_type = self.infer_type(&cmp.left, names);
        let ct_first = if cmp.comparators.is_empty() { prev_type.clone() } else { self.promote(prev_type.clone(), self.infer_type(&cmp.comparators[0], names)) };
        self.compile_expr(&cmp.left, Some(&ct_first), body, locals, names, gs)?;
        prev_type = ct_first;
        let mut prev_tmp = self.alloc_temp(locals, names, prev_type.clone());
        body.push(Instruction::LocalSet(prev_tmp));

        for (i, (op, right)) in cmp.ops.iter().zip(cmp.comparators.iter()).enumerate() {
            let rt = self.infer_type(right, names);
            let ct = self.promote(prev_type.clone(), rt.clone());
            self.compile_expr(right, Some(&ct), body, locals, names, gs)?;
            let right_tmp = self.alloc_temp(locals, names, ct.clone());
            body.push(Instruction::LocalSet(right_tmp));

            body.push(Instruction::LocalGet(prev_tmp));
            self.emit_coercion(body, prev_type.clone(), ct.clone());
            body.push(Instruction::LocalGet(right_tmp));
            // no need to coerce right_tmp because it's already ct

            if let WasmType::Struct(ref name) = ct {
                if let Some(ei) = self.enum_types.get(name).cloned() {
                    let tb = self.alloc_temp(locals, names, ct.clone());
                    body.push(Instruction::LocalSet(tb));
                    body.push(Instruction::StructGet {
                        struct_type_index: ei.type_idx,
                        field_index: 0,
                    });
                    body.push(Instruction::LocalGet(tb));
                    body.push(Instruction::StructGet {
                        struct_type_index: ei.type_idx,
                        field_index: 0,
                    });
                    self.emit_cmp(body, *op, WasmType::I64)?;
                } else {
                    self.emit_cmp(body, *op, ct.clone())?;
                }
            } else {
                self.emit_cmp(body, *op, ct.clone())?;
            }
            body.push(Instruction::I64ExtendI32U);

            if i > 0 {
                body.push(Instruction::I64And);
            }
            prev_tmp = right_tmp;
            prev_type = ct;
        }
        Ok(())
    }

    fn compile_float_mod<'a>(
        &mut self,
        left: &Expr,
        right: &Expr,
        lt: WasmType,
        rt: WasmType,
        body: &mut Vec<Instruction<'a>>,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        gs: &mut Vec<String>,
    ) -> Result<(), String> {
        let ta = self.alloc_temp(locals, names, WasmType::F64);
        let tb = self.alloc_temp(locals, names, WasmType::F64);
        self.compile_expr(left, None, body, locals, names, gs)?;
        self.emit_coercion(body, lt, WasmType::F64);
        body.push(Instruction::LocalSet(ta));
        self.compile_expr(right, None, body, locals, names, gs)?;
        self.emit_coercion(body, rt, WasmType::F64);
        body.push(Instruction::LocalSet(tb));
        body.push(Instruction::LocalGet(ta));
        body.push(Instruction::LocalGet(ta));
        body.push(Instruction::LocalGet(tb));
        body.push(Instruction::F64Div);
        body.push(Instruction::F64Floor);
        body.push(Instruction::LocalGet(tb));
        body.push(Instruction::F64Mul);
        body.push(Instruction::F64Sub);
        Ok(())
    }

    fn alloc_temp(
        &mut self,
        locals: &mut Vec<(u32, ValType)>,
        names: &mut Vec<(String, WasmType)>,
        ty: WasmType,
    ) -> u32 {
        let i = names.len() as u32;
        let vt = self.wasm_type_to_val_type(&ty);
        locals.push((1, vt));
        names.push(("__tmp".to_string(), ty));
        i
    }

    fn promote(&self, a: WasmType, b: WasmType) -> WasmType {
        if a == WasmType::F64 || b == WasmType::F64 {
            WasmType::F64
        } else if a == WasmType::I64 && matches!(b, WasmType::Struct(_)) {
            if let WasmType::Struct(name) = &b {
                if self.enum_types.contains_key(name) {
                    return WasmType::I64;
                }
            }
            b
        } else if b == WasmType::I64 && matches!(a, WasmType::Struct(_)) {
            if let WasmType::Struct(name) = &a {
                if self.enum_types.contains_key(name) {
                    return WasmType::I64;
                }
            }
            a
        } else if matches!(a, WasmType::List(_) | WasmType::Array(_) | WasmType::String | WasmType::Struct(_) | WasmType::None | WasmType::Callable { .. }) {
            a
        } else {
            WasmType::I64
        }
    }

    fn emit_coercion<'a>(&self, body: &mut Vec<Instruction<'a>>, from: WasmType, to: WasmType) {
        if from == WasmType::I64 && to == WasmType::F64 {
            body.push(Instruction::F64ConvertI64S);
        } else if let WasmType::Struct(name) = &from {
            if to == WasmType::I64 {
                if let Some(ei) = self.enum_types.get(name) {
                    body.push(Instruction::StructGet {
                        struct_type_index: ei.type_idx,
                        field_index: 0,
                    });
                }
            }
        }
    }

    fn emit_binop<'a>(
        &self,
        op: ruff_python_ast::Operator,
        t: WasmType,
        body: &mut Vec<Instruction<'a>>,
    ) -> Result<(), String> {
        match (op, t) {
            (ruff_python_ast::Operator::Add, WasmType::F64) => body.push(Instruction::F64Add),
            (ruff_python_ast::Operator::Add, WasmType::I64) => body.push(Instruction::I64Add),
            (ruff_python_ast::Operator::Sub, WasmType::F64) => body.push(Instruction::F64Sub),
            (ruff_python_ast::Operator::Sub, WasmType::I64) => body.push(Instruction::I64Sub),
            (ruff_python_ast::Operator::Mult, WasmType::F64) => body.push(Instruction::F64Mul),
            (ruff_python_ast::Operator::Mult, WasmType::I64) => body.push(Instruction::I64Mul),
            (ruff_python_ast::Operator::Div, WasmType::F64) => body.push(Instruction::F64Div),
            (ruff_python_ast::Operator::FloorDiv, WasmType::I64) => body.push(Instruction::I64DivS),
            (ruff_python_ast::Operator::FloorDiv, WasmType::F64) => {
                body.push(Instruction::F64Div);
                body.push(Instruction::F64Floor);
            }
            (ruff_python_ast::Operator::Mod, WasmType::I64) => body.push(Instruction::I64RemS),
            _ => return Err("Unsupported operator/type combination".to_string()),
        }
        Ok(())
    }

    fn emit_cmp<'a>(
        &mut self,
        body: &mut Vec<Instruction<'a>>,
        op: ruff_python_ast::CmpOp,
        t: WasmType,
    ) -> Result<(), String> {
        if let WasmType::Struct(ref name) = t {
            if let Some(ei) = self.enum_types.get(name).cloned() {
                // To compare enums, we need to compare their .value
                // But emit_cmp assumes the values are already on the stack.
                // This is tricky because we have TWO enum objects on the stack.
                // We need to: temp_set b, struct.get a.0, local.get b, struct.get b.0, then compare.
                
                // Let's simplify: if it's an enum, we handle it in compile_single_cmp
                // and compile_chained_cmp by calling .value on them before calling emit_cmp(I64)
                return Err("emit_cmp called with Enum type - should be handled by caller".to_string());
            }
        }

        match (op, t) {
            (ruff_python_ast::CmpOp::Eq, WasmType::F64) => body.push(Instruction::F64Eq),
            (ruff_python_ast::CmpOp::Eq, WasmType::I64) => body.push(Instruction::I64Eq),
            (
                ruff_python_ast::CmpOp::Eq,
                WasmType::String,
            ) => {
                if let Some(idx) = self.string_eq_idx {
                    body.push(Instruction::Call(idx));
                } else {
                    body.push(Instruction::RefEq);
                }
            }
            (
                ruff_python_ast::CmpOp::Eq,
                WasmType::List(_) | WasmType::Struct(_) | WasmType::None | WasmType::Callable { .. },
            ) => {
                body.push(Instruction::RefEq);
            }
            (ruff_python_ast::CmpOp::NotEq, WasmType::F64) => body.push(Instruction::F64Ne),
            (ruff_python_ast::CmpOp::NotEq, WasmType::I64) => body.push(Instruction::I64Ne),
            (ruff_python_ast::CmpOp::NotEq, WasmType::String) => {
                if let Some(idx) = self.string_eq_idx {
                    body.push(Instruction::Call(idx));
                } else {
                    body.push(Instruction::RefEq);
                }
                body.push(Instruction::I32Eqz);
            }
            (
                ruff_python_ast::CmpOp::NotEq,
                WasmType::List(_) | WasmType::Struct(_) | WasmType::None | WasmType::Callable { .. },
            ) => {
                body.push(Instruction::RefEq);
                body.push(Instruction::I32Eqz);
            }
            (ruff_python_ast::CmpOp::Lt, WasmType::F64) => body.push(Instruction::F64Lt),
            (ruff_python_ast::CmpOp::Lt, WasmType::I64) => body.push(Instruction::I64LtS),
            (ruff_python_ast::CmpOp::LtE, WasmType::F64) => body.push(Instruction::F64Le),
            (ruff_python_ast::CmpOp::LtE, WasmType::I64) => body.push(Instruction::I64LeS),
            (ruff_python_ast::CmpOp::Gt, WasmType::F64) => body.push(Instruction::F64Gt),
            (ruff_python_ast::CmpOp::Gt, WasmType::I64) => body.push(Instruction::I64GtS),
            (ruff_python_ast::CmpOp::GtE, WasmType::F64) => body.push(Instruction::F64Ge),
            (ruff_python_ast::CmpOp::GtE, WasmType::I64) => body.push(Instruction::I64GeS),
            _ => return Err("Unsupported comparison".to_string()),
        }
        Ok(())
    }

    fn infer_type(&self, expr: &Expr, names: &[(String, WasmType)]) -> WasmType {
        match expr {
            Expr::NumberLiteral(l) => match l.value {
                ruff_python_ast::Number::Float(_) => WasmType::F64,
                _ => WasmType::I64,
            },
            Expr::BooleanLiteral(_) | Expr::Compare(_) | Expr::BoolOp(_) => WasmType::I64,
            Expr::StringLiteral(_) => WasmType::String,
            Expr::Attribute(attr) => {
                if let Expr::Name(name) = attr.value.as_ref() {
                    if self.enum_types.contains_key(name.id.as_str()) {
                        return WasmType::Struct(name.id.to_string()); // Correctly inferred as the Enum type
                    }
                }
                let vt = self.infer_type(&attr.value, names);
                if let WasmType::Struct(name) = vt {
                    if self.enum_types.contains_key(&name) {
                        if attr.attr.as_str() == "value" {
                            return WasmType::I64;
                        } else if attr.attr.as_str() == "name" {
                            return WasmType::String;
                        }
                    }
                    if let Some(si) = self.struct_types.get(&name) {
                        if let Some((_, ft)) = si.fields.iter().find(|(n, _)| n == attr.attr.as_str())
                        {
                            return ft.clone();
                        }
                    }
                }
                WasmType::I64
            }
            Expr::Name(n) => {
                if let Some((_, t)) = names.iter().find(|(nm, _)| nm == n.id.as_str()) {
                    return t.clone();
                }
                if let Some(gi) = self.find_global(&n.id) {
                    return gi.wasm_type.clone();
                }
                if let Some(fi) = self.func_infos.iter().find(|fi| fi.name == n.id.as_str()) {
                    return WasmType::Callable {
                        params: fi.param_types.clone(),
                        ret: Box::new(fi.return_type.clone()),
                    };
                }
                WasmType::I64
            }
            Expr::BinOp(b) => {
                if b.op == ruff_python_ast::Operator::Div {
                    WasmType::F64
                } else {
                    self.promote(
                        self.infer_type(&b.left, names),
                        self.infer_type(&b.right, names),
                    )
                }
            }
            Expr::UnaryOp(u) => match u.op {
                ruff_python_ast::UnaryOp::Not => WasmType::I64,
                _ => self.infer_type(&u.operand, names),
            },
            Expr::Call(c) => {
                let func_type = self.infer_type(&c.func, names);
                if let WasmType::Callable { ret, .. } = func_type {
                    return *ret;
                }
                if let Expr::Attribute(attr) = c.func.as_ref() {
                    let obj_type = self.infer_type(&attr.value, names);
                    if let WasmType::List(_) = obj_type {
                        if attr.attr.as_str() == "append" {
                            return WasmType::None;
                        }
                    }
                    if let WasmType::Struct(class_name) = obj_type {
                        if let Some(si) = self.struct_types.get(&class_name) {
                            if let Some(&func_idx) = si.methods.get(attr.attr.as_str()) {
                                return self.func_infos[func_idx as usize].return_type.clone();
                            }
                        }
                    }
                }
                if let Expr::Name(f) = c.func.as_ref() {
                    self.func_infos
                        .iter()
                        .find(|fi| fi.name == f.id.as_str())
                        .map(|fi| fi.return_type.clone())
                        .unwrap_or(WasmType::I64)
                } else {
                    WasmType::I64
                }
            }
            Expr::List(l) => {
                let et = if !l.elts.is_empty() {
                    self.infer_type(&l.elts[0], names)
                } else {
                    WasmType::I64
                };
                WasmType::List(Box::new(et))
            }
            Expr::Dict(d) => {
                let (kt, vt) = if !d.items.is_empty() {
                    (
                        self.infer_type(d.items[0].key.as_ref().unwrap(), names),
                        self.infer_type(&d.items[0].value, names),
                    )
                } else {
                    (WasmType::I64, WasmType::I64)
                };
                WasmType::Dict(Box::new(kt), Box::new(vt))
            }
            Expr::Set(s) => {
                let et = if !s.elts.is_empty() {
                    self.infer_type(&s.elts[0], names)
                } else {
                    WasmType::I64
                };
                WasmType::Set(Box::new(et))
            }
            Expr::ListComp(lc) => {
                let mut comp_names = names.to_vec();
                for generator in &lc.generators {
                    let iter_type = self.infer_type(&generator.iter, names);
                    let gen_elem_type = if let WasmType::List(et) = iter_type {
                        et.as_ref().clone()
                    } else {
                        WasmType::I64
                    };
                    if let Expr::Name(ref name) = generator.target {
                        comp_names.push((name.id.to_string(), gen_elem_type));
                    }
                }
                let et = self.infer_type(&lc.elt, &comp_names);
                WasmType::List(Box::new(et))
            }
            Expr::Subscript(sub) => {
                let vt = self.infer_type(&sub.value, names);
                match vt {
                    WasmType::List(et) => *et,
                    WasmType::String => WasmType::I64, // Assume i64 for codepoint for now
                    _ => WasmType::I64,
                }
            }
            Expr::Lambda(lambda) => {
                let mut param_types = Vec::new();
                let mut lambda_names = names.to_vec();
                if let Some(params) = &lambda.parameters {
                    for p in params.posonlyargs.iter().chain(params.args.iter()) {
                        param_types.push(WasmType::I64);
                        lambda_names.push((p.parameter.name.to_string(), WasmType::I64));
                    }
                }
                let ret = self.infer_type(&lambda.body, &lambda_names);
                WasmType::Callable {
                    params: param_types,
                    ret: Box::new(ret),
                }
            }
            _ => WasmType::I64,
        }
    }

    fn finish(mut self) -> Vec<u8> {
        for info in &self.func_infos {
            self.functions.function(info.type_idx);
            self.codes.function(info.body.as_ref().expect("Function body not compiled"));
        }

        let mut m = Module::new();
        m.section(&self.types);
        m.section(&self.functions);
        if !self.global_infos.is_empty() {
            m.section(&self.globals_sec);
        }
        m.section(&self.exports);
        m.section(&self.codes);
        let mut ns = NameSection::new();
        ns.module("spython");
        ns.functions(&self.func_names);
        ns.locals(&self.func_locals);
        m.section(&ns);
        m.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promote() {
        let c = Compiler::new();
        assert_eq!(c.promote(WasmType::I64, WasmType::I64), WasmType::I64);
        assert_eq!(c.promote(WasmType::I64, WasmType::F64), WasmType::F64);
        assert_eq!(c.promote(WasmType::F64, WasmType::I64), WasmType::F64);
        assert_eq!(c.promote(WasmType::F64, WasmType::F64), WasmType::F64);
        assert_eq!(c.promote(WasmType::String, WasmType::String), WasmType::String);
        assert_eq!(c.promote(WasmType::String, WasmType::I64), WasmType::String);
    }
}
