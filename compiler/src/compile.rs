use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use ruff_db::diagnostic::Severity;
use ruff_db::files::{system_path_to_file, File};
use ruff_db::parsed::parsed_module;
use ruff_db::system::{InMemorySystem, SystemPath, SystemPathBuf, WritableSystem};
use ruff_python_ast::name::Name;
use ruff_python_ast::{BoolOp, CmpOp, Expr, ExprRef, Operator, Stmt, StmtImport, UnaryOp};
use ty_project::{Db, ProjectDatabase, ProjectMetadata};
use ty_python_semantic::{HasType, SemanticModel};
use walrus::ir::{
    ArrayCopy, ArrayGetU, ArrayNewData, BinaryOp, Block, Br, BrIf, GlobalGet, GlobalSet, IfElse,
    Loop, Return, UnaryOp as WasmUnaryOp, Value as WasmValue,
};
use walrus::{
    ConstExpr, DataId, DataKind, FieldType, FunctionBuilder, FunctionId, GlobalId, HeapType,
    InstrSeqBuilder, LocalId, Module, RefType, StorageType, TypeId, ValType,
};

const PROJECT_ROOT: &str = "/";
const USER_FILE: &str = "/user.py";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueType {
    Int,
    Float,
    Bool,
    Str,
}

impl ValueType {
    fn wasm_type(self, string_ty: TypeId) -> ValType {
        match self {
            ValueType::Int => ValType::I64,
            ValueType::Float => ValType::F64,
            ValueType::Bool => ValType::I32,
            ValueType::Str => string_val_type(string_ty),
        }
    }
}

#[derive(Clone, Debug)]
struct FunctionSignature {
    name: String,
    params: Vec<(String, ValueType)>,
    result: ValueType,
}

#[derive(Clone, Copy, Debug)]
struct LocalBinding {
    id: LocalId,
    ty: ValueType,
}

#[derive(Clone, Copy, Debug)]
struct GlobalBinding {
    id: GlobalId,
    ty: ValueType,
}

#[derive(Clone, Copy, Debug)]
enum Binding {
    Local(LocalBinding),
    Global(GlobalBinding),
}

impl Binding {
    fn ty(self) -> ValueType {
        match self {
            Binding::Local(binding) => binding.ty,
            Binding::Global(binding) => binding.ty,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RuntimeHelpers {
    floor_div_i64: FunctionId,
    mod_i64: FunctionId,
    abs_i64: FunctionId,
    min_i64: FunctionId,
    max_i64: FunctionId,
    min_f64: FunctionId,
    max_f64: FunctionId,
    ceil_f64_to_i64: FunctionId,
    pow_f64_i64: FunctionId,
    round_f64_to_i64: FunctionId,
    round_f64_digits: FunctionId,
}

#[derive(Clone, Copy, Debug)]
struct StringLiteralData {
    data: DataId,
    len: u32,
}

#[derive(Clone, Copy, Debug)]
struct StringRuntime {
    str_ty: TypeId,
    str_len: FunctionId,
    str_concat: FunctionId,
    str_repeat: FunctionId,
    str_eq: FunctionId,
    str_slice: FunctionId,
    str_char_at: FunctionId,
    str_upper: FunctionId,
    str_lower: FunctionId,
    int_to_str: FunctionId,
}

pub struct CompileOutput {
    pub wasm: Vec<u8>,
}

#[derive(Debug)]
pub enum CompileError {
    Database(String),
    TypeCheck(Vec<String>),
    Unsupported(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Database(message) => f.write_str(message),
            CompileError::TypeCheck(messages) => {
                write!(f, "type checking failed:\n{}", messages.join("\n"))
            }
            CompileError::Unsupported(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile_source_to_wasm(source: &str) -> Result<CompileOutput, CompileError> {
    let checked = CheckedModule::new(source)?;
    let parsed = parsed_module(&checked.db, checked.file);
    let module = parsed.load(&checked.db);
    let semantic = SemanticModel::new(&checked.db, checked.file);
    let mut compiler = ModuleCompiler::new(&checked.db, &semantic);
    let wasm = compiler.compile_module(module.suite())?;
    Ok(CompileOutput { wasm })
}

pub fn compile_file_to_wasm(path: &Path) -> Result<CompileOutput, CompileError> {
    let source = std::fs::read_to_string(path)
        .map_err(|err| CompileError::Database(format!("cannot read {}: {err}", path.display())))?;
    compile_source_to_wasm(&source)
}

struct CheckedModule {
    db: ProjectDatabase,
    file: File,
}

impl CheckedModule {
    fn new(source: &str) -> Result<Self, CompileError> {
        let cwd = SystemPathBuf::from(PROJECT_ROOT);
        let system = InMemorySystem::new(cwd.clone());
        system
            .write_file(SystemPath::new(USER_FILE), source)
            .map_err(|err| CompileError::Database(err.to_string()))?;

        let metadata = ProjectMetadata::new(Name::new("spython"), cwd);
        let mut db = ProjectDatabase::new(metadata, system)
            .map_err(|err| CompileError::Database(err.to_string()))?;

        let file_path = SystemPathBuf::from(USER_FILE);
        let file = system_path_to_file(&db, &file_path).map_err(|err| {
            CompileError::Database(format!("failed to resolve in-memory user file: {err}"))
        })?;
        db.project().set_included_paths(&mut db, vec![file_path]);

        let diagnostics = db.check();
        let errors: Vec<String> = diagnostics
            .into_iter()
            .filter(|diag| diag.severity() == Severity::Error)
            .map(|diag| format!("{}: {}", diag.id(), diag.primary_message()))
            .collect();
        if !errors.is_empty() {
            return Err(CompileError::TypeCheck(errors));
        }

        Ok(Self { db, file })
    }
}

struct ModuleCompiler<'db> {
    db: &'db ProjectDatabase,
    semantic: &'db SemanticModel<'db>,
    module: Module,
    signatures: BTreeMap<String, FunctionSignature>,
    globals: BTreeMap<String, GlobalBinding>,
    string_literals: BTreeMap<String, StringLiteralData>,
    built_functions: BTreeMap<String, FunctionId>,
    runtime: Option<RuntimeHelpers>,
    string_runtime: Option<StringRuntime>,
}

impl<'db> ModuleCompiler<'db> {
    fn new(db: &'db ProjectDatabase, semantic: &'db SemanticModel<'db>) -> Self {
        Self {
            db,
            semantic,
            module: Module::default(),
            signatures: BTreeMap::new(),
            globals: BTreeMap::new(),
            string_literals: BTreeMap::new(),
            built_functions: BTreeMap::new(),
            runtime: None,
            string_runtime: None,
        }
    }

    fn compile_module(&mut self, suite: &[Stmt]) -> Result<Vec<u8>, CompileError> {
        self.collect_signatures(suite)?;
        self.prepare_string_support(suite)?;
        self.collect_globals(suite)?;
        self.runtime = Some(self.build_runtime_helpers());

        for stmt in suite {
            if let Stmt::FunctionDef(function) = stmt {
                let func_id = self.compile_function(function)?;
                self.built_functions
                    .insert(function.name.as_str().to_owned(), func_id);
            }
        }

        let run = self.compile_run(suite)?;
        self.module.exports.add("run", run);
        Ok(self.module.emit_wasm())
    }

    fn runtime(&self) -> RuntimeHelpers {
        self.runtime.expect("runtime helpers should be built first")
    }

    fn wasm_type(&self, ty: ValueType) -> ValType {
        ty.wasm_type(self.string_runtime().str_ty)
    }

    fn string_runtime(&self) -> StringRuntime {
        self.string_runtime
            .expect("string runtime should be prepared first")
    }

    fn prepare_string_support(&mut self, suite: &[Stmt]) -> Result<(), CompileError> {
        let str_ty = self.module.types.add_array(FieldType {
            element_type: StorageType::I8,
            mutable: true,
        });
        let literals = collect_string_literals(suite);
        for literal in literals {
            if !literal.is_ascii() {
                return Err(CompileError::Unsupported(format!(
                    "non-ASCII string literal is not supported yet: {literal:?}"
                )));
            }
            let len = literal.len() as u32;
            let data = self
                .module
                .data
                .add(DataKind::Passive, literal.clone().into_bytes());
            self.string_literals
                .insert(literal, StringLiteralData { data, len });
        }

        let str_len = self.build_str_len(str_ty);
        let str_concat = self.build_str_concat(str_ty);
        let str_repeat = self.build_str_repeat(str_ty);
        let str_eq = self.build_str_eq(str_ty);
        let str_slice = self.build_str_slice(str_ty);
        let str_char_at = self.build_str_char_at(str_ty);
        let str_upper = self.build_str_upper(str_ty);
        let str_lower = self.build_str_lower(str_ty);
        let int_to_str = self.build_int_to_str(str_ty);

        self.string_runtime = Some(StringRuntime {
            str_ty,
            str_len,
            str_concat,
            str_repeat,
            str_eq,
            str_slice,
            str_char_at,
            str_upper,
            str_lower,
            int_to_str,
        });
        Ok(())
    }

    fn collect_signatures(&mut self, suite: &[Stmt]) -> Result<(), CompileError> {
        for stmt in suite {
            if let Stmt::FunctionDef(function) = stmt {
                if function.is_async {
                    return Err(CompileError::Unsupported(format!(
                        "async function `{}` is not supported",
                        function.name
                    )));
                }
                if function.parameters.vararg.is_some() || function.parameters.kwarg.is_some() {
                    return Err(CompileError::Unsupported(format!(
                        "variadic parameters are not supported in `{}`",
                        function.name
                    )));
                }

                let mut params = Vec::new();
                for parameter in function.parameters.iter_non_variadic_params() {
                    if parameter.default.is_some() {
                        return Err(CompileError::Unsupported(format!(
                            "default arguments are not supported in `{}`",
                            function.name
                        )));
                    }
                    let ty = self.annotation_type(
                        parameter.parameter.annotation.as_deref().ok_or_else(|| {
                            CompileError::Unsupported(format!(
                                "missing parameter annotation in `{}`",
                                function.name
                            ))
                        })?,
                    )?;
                    params.push((parameter.name().as_str().to_owned(), ty));
                }

                let result =
                    self.annotation_type(function.returns.as_deref().ok_or_else(|| {
                        CompileError::Unsupported(format!(
                            "missing return annotation in `{}`",
                            function.name
                        ))
                    })?)?;

                self.signatures.insert(
                    function.name.as_str().to_owned(),
                    FunctionSignature {
                        name: function.name.as_str().to_owned(),
                        params,
                        result,
                    },
                );
            }
        }
        Ok(())
    }

    fn collect_globals(&mut self, suite: &[Stmt]) -> Result<(), CompileError> {
        for stmt in suite {
            match stmt {
                Stmt::FunctionDef(_) | Stmt::Assert(_) => {}
                Stmt::Expr(expr) if expr.value.is_string_literal_expr() => {}
                Stmt::Import(import) => validate_math_import(import)?,
                Stmt::Assign(assign) => {
                    if assign.targets.len() != 1 {
                        return Err(CompileError::Unsupported(
                            "multiple assignment targets are not supported".to_owned(),
                        ));
                    }
                    let name = name_target(&assign.targets[0])?;
                    let ty = self.expr_type(&assign.value)?;
                    self.insert_global_type(&name, ty)?;
                }
                Stmt::AnnAssign(assign) => {
                    let name = name_target(&assign.target)?;
                    let ty = self.annotation_type(&assign.annotation)?;
                    if let Some(value) = assign.value.as_deref() {
                        ensure_assignable_type(self.expr_type(value)?, ty, "annotated global")?;
                    }
                    self.insert_global_type(&name, ty)?;
                }
                other => {
                    return Err(CompileError::Unsupported(format!(
                        "unsupported top-level statement: {:?}",
                        other
                    )));
                }
            }
        }
        Ok(())
    }

    fn insert_global_type(&mut self, name: &str, ty: ValueType) -> Result<(), CompileError> {
        if let Some(existing) = self.globals.get(name) {
            if !is_assignable_type(ty, existing.ty) {
                return Err(CompileError::Unsupported(format!(
                    "global `{name}` changes type from {:?} to {:?}",
                    existing.ty, ty
                )));
            }
            return Ok(());
        }

        let id = self
            .module
            .globals
            .add_local(self.wasm_type(ty), true, false, zero_const_expr(ty, self.string_runtime().str_ty));
        self.module.globals.get_mut(id).name = Some(name.to_owned());
        self.globals.insert(name.to_owned(), GlobalBinding { id, ty });
        Ok(())
    }

    fn compile_function(
        &mut self,
        function: &ruff_python_ast::StmtFunctionDef,
    ) -> Result<FunctionId, CompileError> {
        let signature = self
            .signatures
            .get(function.name.as_str())
            .cloned()
            .ok_or_else(|| {
                CompileError::Unsupported(format!(
                    "missing collected signature for `{}`",
                    function.name
                ))
            })?;

        let mut bindings = BTreeMap::new();
        let mut params = Vec::new();
        let param_types: Vec<ValType> = signature
            .params
            .iter()
            .map(|(_, ty)| self.wasm_type(*ty))
            .collect();
        let result_types = [self.wasm_type(signature.result)];
        let mut builder = FunctionBuilder::new(&mut self.module.types, &param_types, &result_types);
        builder.name(signature.name.clone());

        for (name, ty) in &signature.params {
            let id = self.module.locals.add(self.wasm_type(*ty));
            params.push(id);
            bindings.insert(name.clone(), LocalBinding { id, ty: *ty });
        }

        let local_types = self.collect_local_types(&function.body, &bindings)?;
        for (name, ty) in local_types {
            let id = self.module.locals.add(self.wasm_type(ty));
            bindings.insert(name, LocalBinding { id, ty });
        }

        let mut codegen = FunctionCodegen {
            db: self.db,
            semantic: self.semantic,
            signatures: &self.signatures,
            globals: &self.globals,
            string_literals: &self.string_literals,
            built_functions: &self.built_functions,
            runtime: self.runtime(),
            string_runtime: self.string_runtime(),
            bindings,
            result_type: Some(signature.result),
        };

        {
            let mut body = builder.func_body();
            codegen.compile_stmts(&mut body, &function.body)?;
        }

        Ok(builder.finish(params, &mut self.module.funcs))
    }

    fn compile_run(&mut self, suite: &[Stmt]) -> Result<FunctionId, CompileError> {
        let mut builder = FunctionBuilder::new(&mut self.module.types, &[], &[ValType::I32]);
        builder.name("run".to_owned());

        let mut codegen = FunctionCodegen {
            db: self.db,
            semantic: self.semantic,
            signatures: &self.signatures,
            globals: &self.globals,
            string_literals: &self.string_literals,
            built_functions: &self.built_functions,
            runtime: self.runtime(),
            string_runtime: self.string_runtime(),
            bindings: BTreeMap::new(),
            result_type: None,
        };

        {
            let mut body = builder.func_body();
            let mut assert_index = 0;
            for stmt in suite {
                match stmt {
                    Stmt::FunctionDef(_) => {}
                    Stmt::Assert(assert_stmt) => {
                        assert_index += 1;
                        codegen.compile_assert(&mut body, &assert_stmt.test, assert_index)?;
                    }
                    other => codegen.compile_top_level_stmt(&mut body, other)?,
                }
            }
            body.i32_const(0);
        }

        Ok(builder.finish(vec![], &mut self.module.funcs))
    }

    fn collect_local_types(
        &self,
        stmts: &[Stmt],
        bindings: &BTreeMap<String, LocalBinding>,
    ) -> Result<BTreeMap<String, ValueType>, CompileError> {
        let param_names: BTreeSet<String> = bindings.keys().cloned().collect();
        let mut locals = BTreeMap::new();
        self.collect_local_types_in_stmts(stmts, &param_names, &mut locals)?;
        Ok(locals)
    }

    fn collect_local_types_in_stmts(
        &self,
        stmts: &[Stmt],
        param_names: &BTreeSet<String>,
        locals: &mut BTreeMap<String, ValueType>,
    ) -> Result<(), CompileError> {
        for stmt in stmts {
            match stmt {
                Stmt::Assign(assign) => {
                    if assign.targets.len() != 1 {
                        return Err(CompileError::Unsupported(
                            "multiple assignment targets are not supported".to_owned(),
                        ));
                    }
                    let name = name_target(&assign.targets[0])?;
                    if param_names.contains(&name) {
                        continue;
                    }
                    self.insert_local_type(locals, &name, self.expr_type(&assign.value)?)?;
                }
                Stmt::AnnAssign(assign) => {
                    let name = name_target(&assign.target)?;
                    if param_names.contains(&name) {
                        continue;
                    }
                    let ty = self.annotation_type(&assign.annotation)?;
                    if let Some(value) = assign.value.as_deref() {
                        ensure_assignable_type(self.expr_type(value)?, ty, "annotated local")?;
                    }
                    self.insert_local_type(locals, &name, ty)?;
                }
                Stmt::If(if_stmt) => {
                    self.collect_local_types_in_stmts(&if_stmt.body, param_names, locals)?;
                    for clause in &if_stmt.elif_else_clauses {
                        self.collect_local_types_in_stmts(&clause.body, param_names, locals)?;
                    }
                }
                Stmt::Return(_)
                | Stmt::Assert(_)
                | Stmt::Expr(_)
                | Stmt::Pass(_)
                | Stmt::Break(_)
                | Stmt::Continue(_) => {}
                other => {
                    return Err(CompileError::Unsupported(format!(
                        "unsupported statement in function body: {:?}",
                        other
                    )));
                }
            }
        }
        Ok(())
    }

    fn insert_local_type(
        &self,
        locals: &mut BTreeMap<String, ValueType>,
        name: &str,
        ty: ValueType,
    ) -> Result<(), CompileError> {
        if let Some(existing) = locals.get(name) {
            if !is_assignable_type(ty, *existing) {
                return Err(CompileError::Unsupported(format!(
                    "local `{name}` changes type from {:?} to {:?}",
                    existing, ty
                )));
            }
        } else {
            locals.insert(name.to_owned(), ty);
        }
        Ok(())
    }

    fn annotation_type(&self, expr: &Expr) -> Result<ValueType, CompileError> {
        match expr {
            Expr::Name(name) => match name.id.as_str() {
                "int" => Ok(ValueType::Int),
                "float" => Ok(ValueType::Float),
                "bool" => Ok(ValueType::Bool),
                "str" => Ok(ValueType::Str),
                other => Err(CompileError::Unsupported(format!(
                    "unsupported annotation `{other}`"
                ))),
            },
            other => Err(CompileError::Unsupported(format!(
                "unsupported annotation expression: {:?}",
                other
            ))),
        }
    }

    fn expr_type(&self, expr: &Expr) -> Result<ValueType, CompileError> {
        let ty = ExprRef::from(expr)
            .inferred_type(self.semantic)
            .ok_or_else(|| CompileError::Unsupported("missing inferred type".to_owned()))?;
        let display = simplify_type_display(&ty.display(self.db).to_string());
        match display.as_str() {
            "int" => Ok(ValueType::Int),
            "float" => Ok(ValueType::Float),
            "bool" => Ok(ValueType::Bool),
            "str" => Ok(ValueType::Str),
            "int | float" | "float | int" => Ok(ValueType::Float),
            "Any" | "Unknown" => self.fallback_expr_type(expr),
            other => Err(CompileError::Unsupported(format!(
                "unsupported inferred type `{other}`"
            ))),
        }
    }

    fn fallback_expr_type(&self, expr: &Expr) -> Result<ValueType, CompileError> {
        match expr {
            Expr::StringLiteral(_) => Ok(ValueType::Str),
            Expr::NumberLiteral(number) => match number.value {
                ruff_python_ast::Number::Int(_) => Ok(ValueType::Int),
                ruff_python_ast::Number::Float(_) => Ok(ValueType::Float),
                ruff_python_ast::Number::Complex { .. } => Err(CompileError::Unsupported(
                    "complex numbers are not supported".to_owned(),
                )),
            },
            Expr::BooleanLiteral(_) | Expr::Compare(_) | Expr::BoolOp(_) => Ok(ValueType::Bool),
            Expr::UnaryOp(unary) => match unary.op {
                UnaryOp::Not => Ok(ValueType::Bool),
                UnaryOp::UAdd | UnaryOp::USub => self.expr_type(&unary.operand),
                UnaryOp::Invert => Err(CompileError::Unsupported(
                    "bitwise invert is not supported".to_owned(),
                )),
            },
            Expr::BinOp(binop) => self.fallback_binop_type(binop),
            Expr::Call(call) => self.fallback_call_type(call),
            Expr::Subscript(subscript) => match self.expr_type(&subscript.value)? {
                ValueType::Str => Ok(ValueType::Str),
                other => Err(CompileError::Unsupported(format!(
                    "unsupported subscript base type {:?}",
                    other
                ))),
            },
            other => Err(CompileError::Unsupported(format!(
                "unsupported inferred type `Any` for expression: {:?}",
                other
            ))),
        }
    }

    fn fallback_binop_type(
        &self,
        binop: &ruff_python_ast::ExprBinOp,
    ) -> Result<ValueType, CompileError> {
        let left = self.expr_type(&binop.left)?;
        let right = self.expr_type(&binop.right)?;
        if binop.op == Operator::Add && left == ValueType::Str && right == ValueType::Str {
            return Ok(ValueType::Str);
        }
        if binop.op == Operator::Mult
            && ((left == ValueType::Str && right == ValueType::Int)
                || (left == ValueType::Int && right == ValueType::Str))
        {
            return Ok(ValueType::Str);
        }
        match binop.op {
            Operator::Add | Operator::Sub | Operator::Mult | Operator::Mod => {
                common_numeric_type(left, right)
            }
            Operator::Div => Ok(ValueType::Float),
            Operator::FloorDiv => {
                if left == ValueType::Float || right == ValueType::Float {
                    Ok(ValueType::Float)
                } else {
                    Ok(ValueType::Int)
                }
            }
            Operator::Pow => {
                if left == ValueType::Float || right == ValueType::Float {
                    Ok(ValueType::Float)
                } else {
                    Ok(ValueType::Int)
                }
            }
            other => Err(CompileError::Unsupported(format!(
                "unsupported binary operator `{other}`"
            ))),
        }
    }

    fn fallback_call_type(&self, call: &ruff_python_ast::ExprCall) -> Result<ValueType, CompileError> {
        match &*call.func {
            Expr::Name(name) => {
                if let Some(signature) = self.signatures.get(name.id.as_str()) {
                    return Ok(signature.result);
                }
                fallback_builtin_call_type(
                    name.id.as_str(),
                    |expr| self.expr_type(expr),
                    &call.arguments.args,
                )
            }
            Expr::Attribute(attr) => fallback_attribute_call_type(
                attr,
                |expr| self.expr_type(expr),
                &call.arguments.args,
            ),
            other => Err(CompileError::Unsupported(format!(
                "unsupported callee expression: {:?}",
                other
            ))),
        }
    }

    fn build_runtime_helpers(&mut self) -> RuntimeHelpers {
        let floor_div_i64 = self.build_floor_div_i64();
        let mod_i64 = self.build_mod_i64();
        let abs_i64 = self.build_abs_i64();
        let min_i64 = self.build_min_i64();
        let max_i64 = self.build_max_i64();
        let min_f64 = self.build_min_f64();
        let max_f64 = self.build_max_f64();
        let ceil_f64_to_i64 = self.build_ceil_f64_to_i64();
        let pow_f64_i64 = self.build_pow_f64_i64();
        let round_f64_to_i64 = self.build_round_f64_to_i64();
        let round_f64_digits = self.build_round_f64_digits(pow_f64_i64);
        RuntimeHelpers {
            floor_div_i64,
            mod_i64,
            abs_i64,
            min_i64,
            max_i64,
            min_f64,
            max_f64,
            ceil_f64_to_i64,
            pow_f64_i64,
            round_f64_to_i64,
            round_f64_digits,
        }
    }

    fn build_floor_div_i64(&mut self) -> FunctionId {
        let mut builder = FunctionBuilder::new(
            &mut self.module.types,
            &[ValType::I64, ValType::I64],
            &[ValType::I64],
        );
        builder.name("__sp_floor_div_i64".to_owned());

        let lhs = self.module.locals.add(ValType::I64);
        let rhs = self.module.locals.add(ValType::I64);
        let quotient = self.module.locals.add(ValType::I64);
        let remainder = self.module.locals.add(ValType::I64);

        {
            let mut body = builder.func_body();
            body.local_get(lhs)
                .local_get(rhs)
                .binop(BinaryOp::I64DivS)
                .local_set(quotient);
            body.local_get(lhs)
                .local_get(rhs)
                .binop(BinaryOp::I64RemS)
                .local_set(remainder);
            body.local_get(remainder)
                .i64_const(0)
                .binop(BinaryOp::I64Ne)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .local_get(remainder)
                            .local_get(rhs)
                            .binop(BinaryOp::I64Xor)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |adjust| {
                                    adjust
                                        .local_get(quotient)
                                        .i64_const(1)
                                        .binop(BinaryOp::I64Sub);
                                },
                                |keep| {
                                    keep.local_get(quotient);
                                },
                            );
                    },
                    |else_| {
                        else_.local_get(quotient);
                    },
                );
        }

        builder.finish(vec![lhs, rhs], &mut self.module.funcs)
    }

    fn build_mod_i64(&mut self) -> FunctionId {
        let mut builder = FunctionBuilder::new(
            &mut self.module.types,
            &[ValType::I64, ValType::I64],
            &[ValType::I64],
        );
        builder.name("__sp_mod_i64".to_owned());

        let lhs = self.module.locals.add(ValType::I64);
        let rhs = self.module.locals.add(ValType::I64);
        let remainder = self.module.locals.add(ValType::I64);

        {
            let mut body = builder.func_body();
            body.local_get(lhs)
                .local_get(rhs)
                .binop(BinaryOp::I64RemS)
                .local_set(remainder);
            body.local_get(remainder)
                .i64_const(0)
                .binop(BinaryOp::I64Ne)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .local_get(remainder)
                            .local_get(rhs)
                            .binop(BinaryOp::I64Xor)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |adjust| {
                                    adjust
                                        .local_get(remainder)
                                        .local_get(rhs)
                                        .binop(BinaryOp::I64Add);
                                },
                                |keep| {
                                    keep.local_get(remainder);
                                },
                            );
                    },
                    |else_| {
                        else_.local_get(remainder);
                    },
                );
        }

        builder.finish(vec![lhs, rhs], &mut self.module.funcs)
    }

    fn build_abs_i64(&mut self) -> FunctionId {
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[ValType::I64], &[ValType::I64]);
        builder.name("__sp_abs_i64".to_owned());

        let value = self.module.locals.add(ValType::I64);
        {
            let mut body = builder.func_body();
            body.local_get(value)
                .i64_const(0)
                .binop(BinaryOp::I64LtS)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_.i64_const(0).local_get(value).binop(BinaryOp::I64Sub);
                    },
                    |else_| {
                        else_.local_get(value);
                    },
                );
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }

    fn build_min_i64(&mut self) -> FunctionId {
        self.build_scalar_min_max("__sp_min_i64", ValType::I64, BinaryOp::I64LtS)
    }

    fn build_max_i64(&mut self) -> FunctionId {
        self.build_scalar_min_max("__sp_max_i64", ValType::I64, BinaryOp::I64GtS)
    }

    fn build_min_f64(&mut self) -> FunctionId {
        self.build_scalar_min_max("__sp_min_f64", ValType::F64, BinaryOp::F64Lt)
    }

    fn build_max_f64(&mut self) -> FunctionId {
        self.build_scalar_min_max("__sp_max_f64", ValType::F64, BinaryOp::F64Gt)
    }

    fn build_scalar_min_max(
        &mut self,
        name: &str,
        ty: ValType,
        compare: BinaryOp,
    ) -> FunctionId {
        let mut builder = FunctionBuilder::new(&mut self.module.types, &[ty, ty], &[ty]);
        builder.name(name.to_owned());

        let lhs = self.module.locals.add(ty);
        let rhs = self.module.locals.add(ty);

        {
            let mut body = builder.func_body();
            body.local_get(lhs).local_get(rhs).binop(compare).if_else(
                ty,
                |then_| {
                    then_.local_get(lhs);
                },
                |else_| {
                    else_.local_get(rhs);
                },
            );
        }

        builder.finish(vec![lhs, rhs], &mut self.module.funcs)
    }

    fn build_ceil_f64_to_i64(&mut self) -> FunctionId {
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[ValType::F64], &[ValType::I64]);
        builder.name("__sp_ceil_f64_to_i64".to_owned());

        let value = self.module.locals.add(ValType::F64);
        {
            let mut body = builder.func_body();
            body.local_get(value)
                .unop(WasmUnaryOp::F64Ceil)
                .unop(WasmUnaryOp::I64TruncSF64);
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }

    fn build_pow_f64_i64(&mut self) -> FunctionId {
        let mut builder = FunctionBuilder::new(
            &mut self.module.types,
            &[ValType::F64, ValType::I64],
            &[ValType::F64],
        );
        builder.name("__sp_pow_f64_i64".to_owned());

        let base = self.module.locals.add(ValType::F64);
        let exponent = self.module.locals.add(ValType::I64);
        let positive_exponent = self.module.locals.add(ValType::I64);
        let result = self.module.locals.add(ValType::F64);
        let counter = self.module.locals.add(ValType::I64);

        {
            let mut body = builder.func_body();
            body.local_get(exponent)
                .i64_const(0)
                .binop(BinaryOp::I64LtS)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .i64_const(0)
                            .local_get(exponent)
                            .binop(BinaryOp::I64Sub);
                    },
                    |else_| {
                        else_.local_get(exponent);
                    },
                )
                .local_set(positive_exponent);
            body.f64_const(1.0).local_set(result);
            body.i64_const(0).local_set(counter);

            let exit_id = {
                let mut exit = body.dangling_instr_seq(None);
                let exit_id = exit.id();
                let loop_id = {
                    let mut loop_body = exit.dangling_instr_seq(None);
                    let loop_id = loop_body.id();

                    loop_body
                        .local_get(counter)
                        .local_get(positive_exponent)
                        .binop(BinaryOp::I64GeS)
                        .instr(BrIf { block: exit_id });
                    loop_body
                        .local_get(result)
                        .local_get(base)
                        .binop(BinaryOp::F64Mul)
                        .local_set(result);
                    loop_body
                        .local_get(counter)
                        .i64_const(1)
                        .binop(BinaryOp::I64Add)
                        .local_set(counter);
                    loop_body.instr(Br { block: loop_id });

                    loop_id
                };
                exit.instr(Loop { seq: loop_id });
                exit_id
            };
            body.instr(Block { seq: exit_id });

            body.local_get(exponent)
                .i64_const(0)
                .binop(BinaryOp::I64LtS)
                .if_else(
                    ValType::F64,
                    |then_| {
                        then_
                            .f64_const(1.0)
                            .local_get(result)
                            .binop(BinaryOp::F64Div);
                    },
                    |else_| {
                        else_.local_get(result);
                    },
                );
        }

        builder.finish(vec![base, exponent], &mut self.module.funcs)
    }

    fn build_round_f64_to_i64(&mut self) -> FunctionId {
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[ValType::F64], &[ValType::I64]);
        builder.name("__sp_round_f64_to_i64".to_owned());

        let value = self.module.locals.add(ValType::F64);
        {
            let mut body = builder.func_body();
            body.local_get(value)
                .unop(WasmUnaryOp::F64Nearest)
                .unop(WasmUnaryOp::I64TruncSF64);
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }

    fn build_round_f64_digits(&mut self, pow_f64_i64: FunctionId) -> FunctionId {
        let mut builder = FunctionBuilder::new(
            &mut self.module.types,
            &[ValType::F64, ValType::I64],
            &[ValType::F64],
        );
        builder.name("__sp_round_f64_digits".to_owned());

        let value = self.module.locals.add(ValType::F64);
        let digits = self.module.locals.add(ValType::I64);
        let scale = self.module.locals.add(ValType::F64);

        {
            let mut body = builder.func_body();
            body.local_get(digits)
                .i64_const(0)
                .binop(BinaryOp::I64GeS)
                .if_else(
                    ValType::F64,
                    |then_| {
                        then_
                            .f64_const(10.0)
                            .local_get(digits)
                            .call(pow_f64_i64)
                            .local_set(scale);
                        then_
                            .local_get(value)
                            .local_get(scale)
                            .binop(BinaryOp::F64Mul)
                            .unop(WasmUnaryOp::F64Nearest)
                            .local_get(scale)
                            .binop(BinaryOp::F64Div);
                    },
                    |else_| {
                        else_
                            .f64_const(10.0)
                            .i64_const(0)
                            .local_get(digits)
                            .binop(BinaryOp::I64Sub)
                            .call(pow_f64_i64)
                            .local_set(scale);
                        else_
                            .local_get(value)
                            .local_get(scale)
                            .binop(BinaryOp::F64Div)
                            .unop(WasmUnaryOp::F64Nearest)
                            .local_get(scale)
                            .binop(BinaryOp::F64Mul);
                    },
                );
        }

        builder.finish(vec![value, digits], &mut self.module.funcs)
    }

    fn build_str_len(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder = FunctionBuilder::new(&mut self.module.types, &[str_ref], &[ValType::I64]);
        builder.name("__sp_str_len".to_owned());

        let value = self.module.locals.add(str_ref);
        {
            let mut body = builder.func_body();
            body.local_get(value)
                .array_len()
                .unop(WasmUnaryOp::I64ExtendUI32);
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }

    fn build_str_eq(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[str_ref, str_ref], &[ValType::I32]);
        builder.name("__sp_str_eq".to_owned());

        let lhs = self.module.locals.add(str_ref);
        let rhs = self.module.locals.add(str_ref);
        let len = self.module.locals.add(ValType::I32);
        let idx = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(ValType::I32);

        {
            let mut body = builder.func_body();
            body.local_get(lhs).array_len().local_set(len);
            body.local_get(rhs)
                .array_len()
                .local_get(len)
                .binop(BinaryOp::I32Ne)
                .if_else(
                    ValType::I32,
                    |then_| {
                        then_.i32_const(0);
                    },
                    |else_| {
                        else_.i32_const(1).local_set(result);
                        else_.i32_const(0).local_set(idx);

                        let exit_id = {
                            let mut exit = else_.dangling_instr_seq(None);
                            let exit_id = exit.id();
                            let loop_id = {
                                let mut loop_body = exit.dangling_instr_seq(None);
                                let loop_id = loop_body.id();

                                loop_body
                                    .local_get(idx)
                                    .local_get(len)
                                    .binop(BinaryOp::I32GeU)
                                    .instr(BrIf { block: exit_id });
                                loop_body
                                    .local_get(lhs)
                                    .local_get(idx)
                                    .instr(ArrayGetU { ty: str_ty });
                                loop_body
                                    .local_get(rhs)
                                    .local_get(idx)
                                    .instr(ArrayGetU { ty: str_ty });
                                loop_body.binop(BinaryOp::I32Ne).if_else(
                                    None,
                                    |then_mismatch| {
                                        then_mismatch.i32_const(0).local_set(result);
                                        then_mismatch.instr(Br { block: exit_id });
                                    },
                                    |_else_| {},
                                );
                                loop_body
                                    .local_get(idx)
                                    .i32_const(1)
                                    .binop(BinaryOp::I32Add)
                                    .local_set(idx);
                                loop_body.instr(Br { block: loop_id });

                                loop_id
                            };
                            exit.instr(Loop { seq: loop_id });
                            exit_id
                        };
                        else_.instr(Block { seq: exit_id });
                        else_.local_get(result);
                    },
                );
        }

        builder.finish(vec![lhs, rhs], &mut self.module.funcs)
    }

    fn build_str_concat(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[str_ref, str_ref], &[str_ref]);
        builder.name("__sp_str_concat".to_owned());

        let lhs = self.module.locals.add(str_ref);
        let rhs = self.module.locals.add(str_ref);
        let lhs_len = self.module.locals.add(ValType::I32);
        let rhs_len = self.module.locals.add(ValType::I32);
        let total_len = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(str_ref);

        {
            let mut body = builder.func_body();
            body.local_get(lhs).array_len().local_set(lhs_len);
            body.local_get(rhs).array_len().local_set(rhs_len);
            body.local_get(lhs_len)
                .local_get(rhs_len)
                .binop(BinaryOp::I32Add)
                .local_set(total_len);
            body.i32_const(0)
                .local_get(total_len)
                .array_new(str_ty)
                .local_set(result);
            body.local_get(result)
                .i32_const(0)
                .local_get(lhs)
                .i32_const(0)
                .local_get(lhs_len)
                .instr(ArrayCopy {
                    dst_ty: str_ty,
                    src_ty: str_ty,
                });
            body.local_get(result)
                .local_get(lhs_len)
                .local_get(rhs)
                .i32_const(0)
                .local_get(rhs_len)
                .instr(ArrayCopy {
                    dst_ty: str_ty,
                    src_ty: str_ty,
                });
            body.local_get(result);
        }

        builder.finish(vec![lhs, rhs], &mut self.module.funcs)
    }

    fn build_str_repeat(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[str_ref, ValType::I64], &[str_ref]);
        builder.name("__sp_str_repeat".to_owned());

        let value = self.module.locals.add(str_ref);
        let count = self.module.locals.add(ValType::I64);
        let count_i32 = self.module.locals.add(ValType::I32);
        let len = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(str_ref);
        let idx = self.module.locals.add(ValType::I32);

        {
            let mut body = builder.func_body();
            body.local_get(count)
                .i64_const(0)
                .binop(BinaryOp::I64LeS)
                .if_else(
                    str_ref,
                    |then_| {
                        emit_empty_string(then_, str_ty);
                    },
                    |else_| {
                        else_
                            .local_get(count)
                            .unop(WasmUnaryOp::I32WrapI64)
                            .local_set(count_i32);
                        else_.local_get(value).array_len().local_set(len);
                        else_
                            .i32_const(0)
                            .local_get(len)
                            .local_get(count_i32)
                            .binop(BinaryOp::I32Mul)
                            .array_new(str_ty)
                            .local_set(result);
                        else_.i32_const(0).local_set(idx);

                        let exit_id = {
                            let mut exit = else_.dangling_instr_seq(None);
                            let exit_id = exit.id();
                            let loop_id = {
                                let mut loop_body = exit.dangling_instr_seq(None);
                                let loop_id = loop_body.id();

                                loop_body
                                    .local_get(idx)
                                    .local_get(count_i32)
                                    .binop(BinaryOp::I32GeU)
                                    .instr(BrIf { block: exit_id });
                                loop_body
                                    .local_get(result)
                                    .local_get(idx)
                                    .local_get(len)
                                    .binop(BinaryOp::I32Mul)
                                    .local_get(value)
                                    .i32_const(0)
                                    .local_get(len)
                                    .instr(ArrayCopy {
                                        dst_ty: str_ty,
                                        src_ty: str_ty,
                                    });
                                loop_body
                                    .local_get(idx)
                                    .i32_const(1)
                                    .binop(BinaryOp::I32Add)
                                    .local_set(idx);
                                loop_body.instr(Br { block: loop_id });

                                loop_id
                            };
                            exit.instr(Loop { seq: loop_id });
                            exit_id
                        };
                        else_.instr(Block { seq: exit_id });
                        else_.local_get(result);
                    },
                );
        }

        builder.finish(vec![value, count], &mut self.module.funcs)
    }

    fn build_str_slice(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder = FunctionBuilder::new(
            &mut self.module.types,
            &[str_ref, ValType::I32, ValType::I64, ValType::I32, ValType::I64],
            &[str_ref],
        );
        builder.name("__sp_str_slice".to_owned());

        let value = self.module.locals.add(str_ref);
        let has_start = self.module.locals.add(ValType::I32);
        let start = self.module.locals.add(ValType::I64);
        let has_end = self.module.locals.add(ValType::I32);
        let end = self.module.locals.add(ValType::I64);
        let len_i32 = self.module.locals.add(ValType::I32);
        let len_i64 = self.module.locals.add(ValType::I64);
        let norm_start = self.module.locals.add(ValType::I64);
        let norm_end = self.module.locals.add(ValType::I64);
        let slice_len = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(str_ref);

        {
            let mut body = builder.func_body();
            body.local_get(value).array_len().local_set(len_i32);
            body.local_get(len_i32)
                .unop(WasmUnaryOp::I64ExtendUI32)
                .local_set(len_i64);

            body.local_get(has_start)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .local_get(start)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |neg| {
                                    neg.local_get(len_i64)
                                        .local_get(start)
                                        .binop(BinaryOp::I64Add);
                                },
                                |non_neg| {
                                    non_neg.local_get(start);
                                },
                            )
                            .local_set(norm_start);
                        then_
                            .local_get(norm_start)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |clamp_low| {
                                    clamp_low.i64_const(0);
                                },
                                |clamp_rest| {
                                    clamp_rest
                                        .local_get(norm_start)
                                        .local_get(len_i64)
                                        .binop(BinaryOp::I64GtS)
                                        .if_else(
                                            ValType::I64,
                                            |clamp_high| {
                                                clamp_high.local_get(len_i64);
                                            },
                                            |keep| {
                                                keep.local_get(norm_start);
                                            },
                                        );
                                },
                            );
                    },
                    |else_| {
                        else_.i64_const(0);
                    },
                )
                .local_set(norm_start);

            body.local_get(has_end)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .local_get(end)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |neg| {
                                    neg.local_get(len_i64)
                                        .local_get(end)
                                        .binop(BinaryOp::I64Add);
                                },
                                |non_neg| {
                                    non_neg.local_get(end);
                                },
                            )
                            .local_set(norm_end);
                        then_
                            .local_get(norm_end)
                            .i64_const(0)
                            .binop(BinaryOp::I64LtS)
                            .if_else(
                                ValType::I64,
                                |clamp_low| {
                                    clamp_low.i64_const(0);
                                },
                                |clamp_rest| {
                                    clamp_rest
                                        .local_get(norm_end)
                                        .local_get(len_i64)
                                        .binop(BinaryOp::I64GtS)
                                        .if_else(
                                            ValType::I64,
                                            |clamp_high| {
                                                clamp_high.local_get(len_i64);
                                            },
                                            |keep| {
                                                keep.local_get(norm_end);
                                            },
                                        );
                                },
                            );
                    },
                    |else_| {
                        else_.local_get(len_i64);
                    },
                )
                .local_set(norm_end);

            body.local_get(norm_end)
                .local_get(norm_start)
                .binop(BinaryOp::I64LeS)
                .if_else(
                    ValType::I32,
                    |then_| {
                        then_.i32_const(0);
                    },
                    |else_| {
                        else_
                            .local_get(norm_end)
                            .local_get(norm_start)
                            .binop(BinaryOp::I64Sub)
                            .unop(WasmUnaryOp::I32WrapI64);
                    },
                )
                .local_set(slice_len);

            body.i32_const(0)
                .local_get(slice_len)
                .array_new(str_ty)
                .local_set(result);
            body.local_get(result)
                .i32_const(0)
                .local_get(value)
                .local_get(norm_start)
                .unop(WasmUnaryOp::I32WrapI64)
                .local_get(slice_len)
                .instr(ArrayCopy {
                    dst_ty: str_ty,
                    src_ty: str_ty,
                });
            body.local_get(result);
        }

        builder.finish(vec![value, has_start, start, has_end, end], &mut self.module.funcs)
    }

    fn build_str_char_at(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder =
            FunctionBuilder::new(&mut self.module.types, &[str_ref, ValType::I64], &[str_ref]);
        builder.name("__sp_str_char_at".to_owned());

        let value = self.module.locals.add(str_ref);
        let index = self.module.locals.add(ValType::I64);
        let len = self.module.locals.add(ValType::I64);
        let norm_index = self.module.locals.add(ValType::I64);
        let byte = self.module.locals.add(ValType::I32);

        {
            let mut body = builder.func_body();
            body.local_get(value)
                .array_len()
                .unop(WasmUnaryOp::I64ExtendUI32)
                .local_set(len);
            body.local_get(index)
                .i64_const(0)
                .binop(BinaryOp::I64LtS)
                .if_else(
                    ValType::I64,
                    |then_| {
                        then_
                            .local_get(len)
                            .local_get(index)
                            .binop(BinaryOp::I64Add);
                    },
                    |else_| {
                        else_.local_get(index);
                    },
                )
                .local_set(norm_index);
            body.local_get(value)
                .local_get(norm_index)
                .unop(WasmUnaryOp::I32WrapI64)
                .instr(ArrayGetU { ty: str_ty })
                .local_set(byte);
            body.local_get(byte).array_new_fixed(str_ty, 1);
        }

        builder.finish(vec![value, index], &mut self.module.funcs)
    }

    fn build_str_upper(&mut self, str_ty: TypeId) -> FunctionId {
        self.build_ascii_case_transform("__sp_str_upper", str_ty, true)
    }

    fn build_str_lower(&mut self, str_ty: TypeId) -> FunctionId {
        self.build_ascii_case_transform("__sp_str_lower", str_ty, false)
    }

    fn build_ascii_case_transform(&mut self, name: &str, str_ty: TypeId, upper: bool) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder = FunctionBuilder::new(&mut self.module.types, &[str_ref], &[str_ref]);
        builder.name(name.to_owned());

        let value = self.module.locals.add(str_ref);
        let len = self.module.locals.add(ValType::I32);
        let idx = self.module.locals.add(ValType::I32);
        let byte = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(str_ref);

        {
            let mut body = builder.func_body();
            body.local_get(value).array_len().local_set(len);
            body.i32_const(0)
                .local_get(len)
                .array_new(str_ty)
                .local_set(result);
            body.i32_const(0).local_set(idx);

            let exit_id = {
                let mut exit = body.dangling_instr_seq(None);
                let exit_id = exit.id();
                let loop_id = {
                    let mut loop_body = exit.dangling_instr_seq(None);
                    let loop_id = loop_body.id();

                    loop_body
                        .local_get(idx)
                        .local_get(len)
                        .binop(BinaryOp::I32GeU)
                        .instr(BrIf { block: exit_id });
                    loop_body
                        .local_get(value)
                        .local_get(idx)
                        .instr(ArrayGetU { ty: str_ty })
                        .local_set(byte);
                    loop_body
                        .local_get(result)
                        .local_get(idx);
                    loop_body
                        .local_get(byte)
                        .i32_const(if upper { b'a' as i32 } else { b'A' as i32 })
                        .binop(BinaryOp::I32GeU);
                    loop_body
                        .local_get(byte)
                        .i32_const(if upper { b'z' as i32 } else { b'Z' as i32 })
                        .binop(BinaryOp::I32LeU);
                    loop_body.binop(BinaryOp::I32And).if_else(
                        ValType::I32,
                        |then_| {
                            then_
                                .local_get(byte)
                                .i32_const(32)
                                .binop(if upper {
                                    BinaryOp::I32Sub
                                } else {
                                    BinaryOp::I32Add
                                });
                        },
                        |else_| {
                            else_.local_get(byte);
                        },
                    );
                    loop_body.array_set(str_ty);
                    loop_body
                        .local_get(idx)
                        .i32_const(1)
                        .binop(BinaryOp::I32Add)
                        .local_set(idx);
                    loop_body.instr(Br { block: loop_id });

                    loop_id
                };
                exit.instr(Loop { seq: loop_id });
                exit_id
            };
            body.instr(Block { seq: exit_id });
            body.local_get(result);
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }

    fn build_int_to_str(&mut self, str_ty: TypeId) -> FunctionId {
        let str_ref = string_val_type(str_ty);
        let mut builder = FunctionBuilder::new(&mut self.module.types, &[ValType::I64], &[str_ref]);
        builder.name("__sp_int_to_str".to_owned());

        let value = self.module.locals.add(ValType::I64);
        let negative = self.module.locals.add(ValType::I32);
        let magnitude = self.module.locals.add(ValType::I64);
        let temp = self.module.locals.add(ValType::I64);
        let digits = self.module.locals.add(ValType::I32);
        let total = self.module.locals.add(ValType::I32);
        let result = self.module.locals.add(str_ref);
        let index = self.module.locals.add(ValType::I32);
        let digit = self.module.locals.add(ValType::I32);

        {
            let mut body = builder.func_body();
            body.local_get(value)
                .i64_const(0)
                .binop(BinaryOp::I64LtS)
                .if_else(
                    ValType::I32,
                    |then_| {
                        then_.i32_const(1);
                    },
                    |else_| {
                        else_.i32_const(0);
                    },
                )
                .local_set(negative);
            body.local_get(negative).if_else(
                ValType::I64,
                |then_| {
                    then_.i64_const(0).local_get(value).binop(BinaryOp::I64Sub);
                },
                |else_| {
                    else_.local_get(value);
                },
            )
            .local_set(magnitude);

            body.local_get(magnitude)
                .i64_const(0)
                .binop(BinaryOp::I64Eq)
                .if_else(
                    None,
                    |then_| {
                        then_.i32_const(1).local_set(digits);
                    },
                    |else_| {
                        else_.i32_const(0).local_set(digits);
                        else_.local_get(magnitude).local_set(temp);

                        let exit_id = {
                            let mut exit = else_.dangling_instr_seq(None);
                            let exit_id = exit.id();
                            let loop_id = {
                                let mut loop_body = exit.dangling_instr_seq(None);
                                let loop_id = loop_body.id();

                                loop_body
                                    .local_get(temp)
                                    .i64_const(0)
                                    .binop(BinaryOp::I64Eq)
                                    .instr(BrIf { block: exit_id });
                                loop_body
                                    .local_get(digits)
                                    .i32_const(1)
                                    .binop(BinaryOp::I32Add)
                                    .local_set(digits);
                                loop_body
                                    .local_get(temp)
                                    .i64_const(10)
                                    .binop(BinaryOp::I64DivS)
                                    .local_set(temp);
                                loop_body.instr(Br { block: loop_id });

                                loop_id
                            };
                            exit.instr(Loop { seq: loop_id });
                            exit_id
                        };
                        else_.instr(Block { seq: exit_id });
                    },
                );

            body.local_get(digits)
                .local_get(negative)
                .binop(BinaryOp::I32Add)
                .local_set(total);
            body.i32_const(0)
                .local_get(total)
                .array_new(str_ty)
                .local_set(result);
            body.local_get(magnitude).local_set(temp);
            body.local_get(total).local_set(index);

            body.local_get(magnitude)
                .i64_const(0)
                .binop(BinaryOp::I64Eq)
                .if_else(
                    None,
                    |then_| {
                        then_
                            .local_get(result)
                            .i32_const(0)
                            .i32_const(b'0' as i32)
                            .array_set(str_ty);
                    },
                    |else_| {
                        let exit_id = {
                            let mut exit = else_.dangling_instr_seq(None);
                            let exit_id = exit.id();
                            let loop_id = {
                                let mut loop_body = exit.dangling_instr_seq(None);
                                let loop_id = loop_body.id();

                                loop_body
                                    .local_get(temp)
                                    .i64_const(0)
                                    .binop(BinaryOp::I64Eq)
                                    .instr(BrIf { block: exit_id });
                                loop_body
                                    .local_get(index)
                                    .i32_const(1)
                                    .binop(BinaryOp::I32Sub)
                                    .local_set(index);
                                loop_body
                                    .local_get(temp)
                                    .i64_const(10)
                                    .binop(BinaryOp::I64RemS)
                                    .unop(WasmUnaryOp::I32WrapI64)
                                    .local_set(digit);
                                loop_body
                                    .local_get(result)
                                    .local_get(index)
                                    .local_get(digit)
                                    .i32_const(b'0' as i32)
                                    .binop(BinaryOp::I32Add)
                                    .array_set(str_ty);
                                loop_body
                                    .local_get(temp)
                                    .i64_const(10)
                                    .binop(BinaryOp::I64DivS)
                                    .local_set(temp);
                                loop_body.instr(Br { block: loop_id });

                                loop_id
                            };
                            exit.instr(Loop { seq: loop_id });
                            exit_id
                        };
                        else_.instr(Block { seq: exit_id });
                    },
                );

            body.local_get(negative).if_else(
                None,
                |then_| {
                    then_
                        .local_get(result)
                        .i32_const(0)
                        .i32_const(b'-' as i32)
                        .array_set(str_ty);
                },
                |_else_| {},
            );
            body.local_get(result);
        }

        builder.finish(vec![value], &mut self.module.funcs)
    }
}

struct FunctionCodegen<'a, 'db> {
    db: &'a ProjectDatabase,
    semantic: &'a SemanticModel<'db>,
    signatures: &'a BTreeMap<String, FunctionSignature>,
    globals: &'a BTreeMap<String, GlobalBinding>,
    string_literals: &'a BTreeMap<String, StringLiteralData>,
    built_functions: &'a BTreeMap<String, FunctionId>,
    runtime: RuntimeHelpers,
    string_runtime: StringRuntime,
    bindings: BTreeMap<String, LocalBinding>,
    result_type: Option<ValueType>,
}

impl<'a, 'db> FunctionCodegen<'a, 'db> {
    fn compile_top_level_stmt(
        &mut self,
        builder: &mut InstrSeqBuilder,
        stmt: &Stmt,
    ) -> Result<(), CompileError> {
        match stmt {
            Stmt::Import(import) => validate_math_import(import),
            Stmt::Expr(expr) if expr.value.is_string_literal_expr() => Ok(()),
            Stmt::Assign(_) | Stmt::AnnAssign(_) | Stmt::Pass(_) => self.compile_stmt(builder, stmt),
            other => Err(CompileError::Unsupported(format!(
                "unsupported top-level statement: {:?}",
                other
            ))),
        }
    }

    fn compile_stmts(
        &mut self,
        builder: &mut InstrSeqBuilder,
        stmts: &[Stmt],
    ) -> Result<(), CompileError> {
        for stmt in stmts {
            self.compile_stmt(builder, stmt)?;
        }
        Ok(())
    }

    fn compile_stmt(
        &mut self,
        builder: &mut InstrSeqBuilder,
        stmt: &Stmt,
    ) -> Result<(), CompileError> {
        match stmt {
            Stmt::Return(ret) => {
                let result_type = self.result_type.ok_or_else(|| {
                    CompileError::Unsupported("return outside function is not supported".to_owned())
                })?;
                let value = ret.value.as_deref().ok_or_else(|| {
                    CompileError::Unsupported("return without value is not supported".to_owned())
                })?;
                self.compile_expr_as(builder, value, result_type)?;
                builder.instr(Return {});
                Ok(())
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() != 1 {
                    return Err(CompileError::Unsupported(
                        "multiple assignment targets are not supported".to_owned(),
                    ));
                }
                let name = name_target(&assign.targets[0])?;
                let binding = self.binding(&name)?;
                self.compile_expr_as(builder, &assign.value, binding.ty())?;
                self.store_binding(builder, binding);
                Ok(())
            }
            Stmt::AnnAssign(assign) => {
                let name = name_target(&assign.target)?;
                let binding = self.binding(&name)?;
                let value = assign.value.as_deref().ok_or_else(|| {
                    CompileError::Unsupported(
                        "annotated assignment without value is not supported".to_owned(),
                    )
                })?;
                self.compile_expr_as(builder, value, binding.ty())?;
                self.store_binding(builder, binding);
                Ok(())
            }
            Stmt::If(if_stmt) => self.compile_if_stmt(
                builder,
                &if_stmt.test,
                &if_stmt.body,
                &if_stmt.elif_else_clauses,
            ),
            Stmt::Expr(expr) if expr.value.is_string_literal_expr() => Ok(()),
            Stmt::Pass(_) => Ok(()),
            other => Err(CompileError::Unsupported(format!(
                "unsupported statement in codegen: {:?}",
                other
            ))),
        }
    }

    fn compile_if_stmt(
        &mut self,
        builder: &mut InstrSeqBuilder,
        test: &Expr,
        body: &[Stmt],
        clauses: &[ruff_python_ast::ElifElseClause],
    ) -> Result<(), CompileError> {
        let ty = self.compile_expr(builder, test)?;
        if ty != ValueType::Bool {
            return Err(CompileError::Unsupported(
                "if condition must be bool".to_owned(),
            ));
        }

        self.emit_if_else(
            builder,
            None,
            |this, then_| this.compile_stmts(then_, body),
            |this, else_| {
                if let Some((first, rest)) = clauses.split_first() {
                    if let Some(test) = &first.test {
                        this.compile_if_stmt(else_, test, &first.body, rest)
                    } else {
                        this.compile_stmts(else_, &first.body)
                    }
                } else {
                    Ok(())
                }
            },
        )
    }

    fn compile_assert(
        &mut self,
        builder: &mut InstrSeqBuilder,
        test: &Expr,
        failure_code: i32,
    ) -> Result<(), CompileError> {
        let ty = self.compile_expr(builder, test)?;
        if ty != ValueType::Bool {
            return Err(CompileError::Unsupported(
                "assert test must be bool".to_owned(),
            ));
        }
        builder.unop(WasmUnaryOp::I32Eqz);
        self.emit_if_else(
            builder,
            None,
            |_this, then_| {
                then_.i32_const(failure_code).instr(Return {});
                Ok(())
            },
            |_this, _else_| Ok(()),
        )
    }

    fn compile_expr(
        &mut self,
        builder: &mut InstrSeqBuilder,
        expr: &Expr,
    ) -> Result<ValueType, CompileError> {
        let ty = self.expr_type(expr)?;
        match expr {
            Expr::StringLiteral(string) => {
                self.emit_string_literal(builder, string)?;
            }
            Expr::NumberLiteral(number) => match &number.value {
                ruff_python_ast::Number::Int(value) => {
                    builder.i64_const(value.as_i64().ok_or_else(|| {
                        CompileError::Unsupported("integer literal does not fit in i64".to_owned())
                    })?);
                }
                ruff_python_ast::Number::Float(value) => {
                    builder.f64_const(*value);
                }
                ruff_python_ast::Number::Complex { .. } => {
                    return Err(CompileError::Unsupported(
                        "complex numbers are not supported".to_owned(),
                    ));
                }
            },
            Expr::BooleanLiteral(boolean) => {
                builder.i32_const(i32::from(boolean.value));
            }
            Expr::Name(name) => {
                let binding = self.binding(name.id.as_str())?;
                self.load_binding(builder, binding);
            }
            Expr::BinOp(binop) => {
                self.compile_binop(builder, binop, ty)?;
            }
            Expr::UnaryOp(unary) => match unary.op {
                UnaryOp::USub => {
                    let operand_ty = self.compile_expr(builder, &unary.operand)?;
                    match operand_ty {
                        ValueType::Int => builder.i64_const(-1).binop(BinaryOp::I64Mul),
                        ValueType::Float => builder.unop(WasmUnaryOp::F64Neg),
                        ValueType::Bool => {
                            return Err(CompileError::Unsupported(
                                "unary minus on bool is not supported".to_owned(),
                            ));
                        }
                        ValueType::Str => {
                            return Err(CompileError::Unsupported(
                                "unary minus on str is not supported".to_owned(),
                            ));
                        }
                    };
                }
                UnaryOp::UAdd => {
                    self.compile_expr(builder, &unary.operand)?;
                }
                UnaryOp::Not => {
                    let operand_ty = self.compile_expr(builder, &unary.operand)?;
                    if operand_ty != ValueType::Bool {
                        return Err(CompileError::Unsupported("`not` expects a bool".to_owned()));
                    }
                    builder.unop(WasmUnaryOp::I32Eqz);
                }
                UnaryOp::Invert => {
                    return Err(CompileError::Unsupported(
                        "bitwise invert is not supported".to_owned(),
                    ));
                }
            },
            Expr::Compare(compare) => {
                self.compile_compare(builder, compare)?;
            }
            Expr::BoolOp(bool_op) => {
                self.compile_bool_expr(builder, bool_op.op, &bool_op.values)?;
            }
            Expr::Call(call) => {
                self.compile_call(builder, call)?;
            }
            Expr::Subscript(subscript) => {
                self.compile_subscript(builder, subscript)?;
            }
            other => {
                return Err(CompileError::Unsupported(format!(
                    "unsupported expression: {:?}",
                    other
                )));
            }
        }
        Ok(ty)
    }

    fn compile_expr_as(
        &mut self,
        builder: &mut InstrSeqBuilder,
        expr: &Expr,
        expected: ValueType,
    ) -> Result<(), CompileError> {
        let actual = self.expr_type(expr)?;
        self.compile_expr(builder, expr)?;
        if actual == expected {
            return Ok(());
        }
        match (actual, expected) {
            (ValueType::Int, ValueType::Float) => {
                builder.unop(WasmUnaryOp::F64ConvertSI64);
                Ok(())
            }
            _ => Err(CompileError::Unsupported(format!(
                "cannot convert {:?} to {:?}",
                actual, expected
            ))),
        }
    }

    fn compile_binop(
        &mut self,
        builder: &mut InstrSeqBuilder,
        binop: &ruff_python_ast::ExprBinOp,
        ty: ValueType,
    ) -> Result<(), CompileError> {
        if ty == ValueType::Str {
            return self.compile_string_binop(builder, binop);
        }
        match (binop.op, ty) {
            (Operator::Pow, ValueType::Float) => {
                self.compile_expr_as(builder, &binop.left, ValueType::Float)?;
                let right_ty = self.expr_type(&binop.right)?;
                match right_ty {
                    ValueType::Int => {
                        self.compile_expr_as(builder, &binop.right, ValueType::Int)?;
                        builder.call(self.runtime.pow_f64_i64);
                    }
                    ValueType::Float if is_float_literal(&binop.right, 0.5) => {
                        builder.unop(WasmUnaryOp::F64Sqrt);
                    }
                    _ => {
                        return Err(CompileError::Unsupported(
                            "unsupported float exponent".to_owned(),
                        ));
                    }
                }
            }
            (Operator::FloorDiv, ValueType::Int) => {
                self.compile_expr_as(builder, &binop.left, ValueType::Int)?;
                self.compile_expr_as(builder, &binop.right, ValueType::Int)?;
                builder.call(self.runtime.floor_div_i64);
            }
            (Operator::Mod, ValueType::Int) => {
                self.compile_expr_as(builder, &binop.left, ValueType::Int)?;
                self.compile_expr_as(builder, &binop.right, ValueType::Int)?;
                builder.call(self.runtime.mod_i64);
            }
            (Operator::FloorDiv, ValueType::Float) => {
                self.compile_expr_as(builder, &binop.left, ValueType::Float)?;
                self.compile_expr_as(builder, &binop.right, ValueType::Float)?;
                builder.binop(BinaryOp::F64Div).unop(WasmUnaryOp::F64Floor);
            }
            _ => {
                self.compile_expr_as(builder, &binop.left, ty)?;
                self.compile_expr_as(builder, &binop.right, ty)?;
                builder.binop(self.binary_op(binop.op, ty)?);
            }
        }
        Ok(())
    }

    fn compile_compare(
        &mut self,
        builder: &mut InstrSeqBuilder,
        compare: &ruff_python_ast::ExprCompare,
    ) -> Result<(), CompileError> {
        if compare.ops.len() != 1 || compare.comparators.len() != 1 {
            return Err(CompileError::Unsupported(
                "comparison chains are not supported".to_owned(),
            ));
        }

        let left_ty = self.expr_type(&compare.left)?;
        let right_ty = self.expr_type(&compare.comparators[0])?;
        let compare_ty = common_compare_type(left_ty, right_ty)?;

        self.compile_expr_as(builder, &compare.left, compare_ty)?;
        self.compile_expr_as(builder, &compare.comparators[0], compare_ty)?;

        match compare.ops[0] {
            CmpOp::Eq if compare_ty == ValueType::Str => {
                builder.call(self.string_runtime.str_eq);
            }
            CmpOp::NotEq if compare_ty == ValueType::Str => {
                builder.call(self.string_runtime.str_eq);
                builder.unop(WasmUnaryOp::I32Eqz);
            }
            CmpOp::NotEq => {
                builder.binop(self.compare_op(CmpOp::Eq, compare_ty)?);
                builder.unop(WasmUnaryOp::I32Eqz);
            }
            op => {
                builder.binop(self.compare_op(op, compare_ty)?);
            }
        }
        Ok(())
    }

    fn compile_call(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        match &*call.func {
            Expr::Name(name) => {
                let callee = name.id.as_str();
                if self.signatures.contains_key(callee) {
                    self.compile_user_function_call(builder, callee, call)
                } else {
                    self.compile_builtin_call(builder, callee, call)
                }
            }
            Expr::Attribute(attr) => self.compile_attribute_call(builder, attr, call),
            other => Err(CompileError::Unsupported(format!(
                "unsupported callee expression: {:?}",
                other
            ))),
        }
    }

    fn compile_user_function_call(
        &mut self,
        builder: &mut InstrSeqBuilder,
        callee: &str,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        let signature = self.signatures.get(callee).ok_or_else(|| {
            CompileError::Unsupported(format!("unknown function `{callee}`"))
        })?;
        if call.arguments.args.len() != signature.params.len() || !call.arguments.keywords.is_empty()
        {
            return Err(CompileError::Unsupported(format!(
                "unsupported call shape for `{callee}`"
            )));
        }
        let func_id = *self.built_functions.get(callee).ok_or_else(|| {
            CompileError::Unsupported(format!(
                "calls to not-yet-built function `{callee}` are not supported"
            ))
        })?;
        let params = signature.params.clone();
        for (arg, (_, param_ty)) in call.arguments.args.iter().zip(&params) {
            self.compile_expr_as(builder, arg, *param_ty)?;
        }
        builder.call(func_id);
        Ok(())
    }

    fn compile_builtin_call(
        &mut self,
        builder: &mut InstrSeqBuilder,
        callee: &str,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        if !call.arguments.keywords.is_empty() {
            return Err(CompileError::Unsupported(format!(
                "keyword arguments are not supported in `{callee}`"
            )));
        }

        match callee {
            "abs" => self.compile_builtin_abs(builder, call),
            "len" => self.compile_builtin_len(builder, call),
            "min" => self.compile_builtin_min_max(builder, call, false),
            "max" => self.compile_builtin_min_max(builder, call, true),
            "round" => self.compile_builtin_round(builder, call),
            "str" => self.compile_builtin_str(builder, call),
            other => Err(CompileError::Unsupported(format!(
                "unsupported builtin `{other}`"
            ))),
        }
    }

    fn compile_builtin_abs(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        let [arg] = &*call.arguments.args else {
            return Err(CompileError::Unsupported(
                "`abs` expects exactly one argument".to_owned(),
            ));
        };
        match self.expr_type(arg)? {
            ValueType::Int => {
                self.compile_expr_as(builder, arg, ValueType::Int)?;
                builder.call(self.runtime.abs_i64);
            }
            ValueType::Float => {
                self.compile_expr_as(builder, arg, ValueType::Float)?;
                builder.unop(WasmUnaryOp::F64Abs);
            }
            ValueType::Bool => {
                return Err(CompileError::Unsupported(
                    "`abs` on bool is not supported".to_owned(),
                ));
            }
            ValueType::Str => {
                return Err(CompileError::Unsupported(
                    "`abs` on str is not supported".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn compile_builtin_len(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        let [arg] = &*call.arguments.args else {
            return Err(CompileError::Unsupported(
                "`len` expects exactly one argument".to_owned(),
            ));
        };
        match self.expr_type(arg)? {
            ValueType::Str => {
                self.compile_expr_as(builder, arg, ValueType::Str)?;
                builder.call(self.string_runtime.str_len);
                Ok(())
            }
            other => Err(CompileError::Unsupported(format!(
                "`len` is not supported for {:?}",
                other
            ))),
        }
    }

    fn compile_builtin_min_max(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
        is_max: bool,
    ) -> Result<(), CompileError> {
        let [lhs, rhs] = &*call.arguments.args else {
            return Err(CompileError::Unsupported(
                "`min`/`max` expect exactly two arguments".to_owned(),
            ));
        };

        let lhs_ty = self.expr_type(lhs)?;
        let rhs_ty = self.expr_type(rhs)?;
        if lhs_ty != rhs_ty {
            return Err(CompileError::Unsupported(
                "`min`/`max` with mixed numeric types are not supported yet".to_owned(),
            ));
        }

        match lhs_ty {
            ValueType::Int => {
                self.compile_expr_as(builder, lhs, ValueType::Int)?;
                self.compile_expr_as(builder, rhs, ValueType::Int)?;
                builder.call(if is_max {
                    self.runtime.max_i64
                } else {
                    self.runtime.min_i64
                });
            }
            ValueType::Float => {
                self.compile_expr_as(builder, lhs, ValueType::Float)?;
                self.compile_expr_as(builder, rhs, ValueType::Float)?;
                builder.call(if is_max {
                    self.runtime.max_f64
                } else {
                    self.runtime.min_f64
                });
            }
            ValueType::Bool => {
                return Err(CompileError::Unsupported(
                    "`min`/`max` on bool are not supported".to_owned(),
                ));
            }
            ValueType::Str => {
                return Err(CompileError::Unsupported(
                    "`min`/`max` on str are not supported".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn compile_builtin_round(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        match &*call.arguments.args {
            [value] => match self.expr_type(value)? {
                ValueType::Int => {
                    self.compile_expr_as(builder, value, ValueType::Int)?;
                }
                ValueType::Float => {
                    self.compile_expr_as(builder, value, ValueType::Float)?;
                    builder.call(self.runtime.round_f64_to_i64);
                }
                ValueType::Bool => {
                    return Err(CompileError::Unsupported(
                        "`round` on bool is not supported".to_owned(),
                    ));
                }
                ValueType::Str => {
                    return Err(CompileError::Unsupported(
                        "`round` on str is not supported".to_owned(),
                    ));
                }
            },
            [value, digits] => {
                if self.expr_type(value)? != ValueType::Float || self.expr_type(digits)? != ValueType::Int
                {
                    return Err(CompileError::Unsupported(
                        "`round(x, n)` currently expects `(float, int)`".to_owned(),
                    ));
                }
                self.compile_expr_as(builder, value, ValueType::Float)?;
                self.compile_expr_as(builder, digits, ValueType::Int)?;
                builder.call(self.runtime.round_f64_digits);
            }
            _ => {
                return Err(CompileError::Unsupported(
                    "`round` expects one or two positional arguments".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn compile_builtin_str(
        &mut self,
        builder: &mut InstrSeqBuilder,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        let [value] = &*call.arguments.args else {
            return Err(CompileError::Unsupported(
                "`str` expects exactly one argument".to_owned(),
            ));
        };
        match self.expr_type(value)? {
            ValueType::Int => {
                self.compile_expr_as(builder, value, ValueType::Int)?;
                builder.call(self.string_runtime.int_to_str);
                Ok(())
            }
            ValueType::Str => self.compile_expr_as(builder, value, ValueType::Str),
            other => Err(CompileError::Unsupported(format!(
                "`str` is not supported for {:?}",
                other
            ))),
        }
    }

    fn compile_attribute_call(
        &mut self,
        builder: &mut InstrSeqBuilder,
        attr: &ruff_python_ast::ExprAttribute,
        call: &ruff_python_ast::ExprCall,
    ) -> Result<(), CompileError> {
        if let Expr::Name(module_name) = &*attr.value
            && module_name.id.as_str() == "math"
            && attr.attr.as_str() == "ceil"
        {
            let [arg] = &*call.arguments.args else {
                return Err(CompileError::Unsupported(
                    "`math.ceil` expects exactly one argument".to_owned(),
                ));
            };
            if !call.arguments.keywords.is_empty() {
                return Err(CompileError::Unsupported(
                    "keyword arguments are not supported in `math.ceil`".to_owned(),
                ));
            }
            self.compile_expr_as(builder, arg, ValueType::Float)?;
            builder.call(self.runtime.ceil_f64_to_i64);
            return Ok(());
        }

        if !call.arguments.args.is_empty() || !call.arguments.keywords.is_empty() {
            return Err(CompileError::Unsupported(format!(
                "unsupported attribute call `{:?}.{}(...)`",
                attr.value, attr.attr
            )));
        }

        let receiver_ty = self.expr_type(&attr.value)?;
        match (receiver_ty, attr.attr.as_str()) {
            (ValueType::Str, "upper") => {
                self.compile_expr_as(builder, &attr.value, ValueType::Str)?;
                builder.call(self.string_runtime.str_upper);
                Ok(())
            }
            (ValueType::Str, "lower") => {
                self.compile_expr_as(builder, &attr.value, ValueType::Str)?;
                builder.call(self.string_runtime.str_lower);
                Ok(())
            }
            _ => Err(CompileError::Unsupported(format!(
                "unsupported attribute call `{:?}.{}()`",
                attr.value, attr.attr
            ))),
        }
    }

    fn compile_bool_expr(
        &mut self,
        builder: &mut InstrSeqBuilder,
        op: BoolOp,
        values: &[Expr],
    ) -> Result<(), CompileError> {
        let Some((first, rest)) = values.split_first() else {
            return Err(CompileError::Unsupported(
                "empty boolean expression".to_owned(),
            ));
        };
        let ty = self.compile_expr(builder, first)?;
        if ty != ValueType::Bool {
            return Err(CompileError::Unsupported(
                "boolean operators require bool operands".to_owned(),
            ));
        }
        if rest.is_empty() {
            return Ok(());
        }

        self.emit_if_else(
            builder,
            Some(ValType::I32),
            |this, then_| match op {
                BoolOp::And => this.compile_bool_expr(then_, op, rest),
                BoolOp::Or => {
                    then_.i32_const(1);
                    Ok(())
                }
            },
            |this, else_| match op {
                BoolOp::And => {
                    else_.i32_const(0);
                    Ok(())
                }
                BoolOp::Or => this.compile_bool_expr(else_, op, rest),
            },
        )
    }

    fn emit_string_literal(
        &mut self,
        builder: &mut InstrSeqBuilder,
        string: &ruff_python_ast::ExprStringLiteral,
    ) -> Result<(), CompileError> {
        let literal = self
            .string_literals
            .get(string.value.to_str())
            .copied()
            .ok_or_else(|| {
                CompileError::Unsupported(format!(
                    "missing interned string literal `{}`",
                    string.value.to_str()
                ))
            })?;
        builder
            .i32_const(0)
            .i32_const(literal.len as i32)
            .instr(ArrayNewData {
                ty: self.string_runtime.str_ty,
                data: literal.data,
            });
        Ok(())
    }

    fn compile_string_binop(
        &mut self,
        builder: &mut InstrSeqBuilder,
        binop: &ruff_python_ast::ExprBinOp,
    ) -> Result<(), CompileError> {
        let left_ty = self.expr_type(&binop.left)?;
        let right_ty = self.expr_type(&binop.right)?;
        match binop.op {
            Operator::Add if left_ty == ValueType::Str && right_ty == ValueType::Str => {
                self.compile_expr_as(builder, &binop.left, ValueType::Str)?;
                self.compile_expr_as(builder, &binop.right, ValueType::Str)?;
                builder.call(self.string_runtime.str_concat);
                Ok(())
            }
            Operator::Mult if left_ty == ValueType::Str && right_ty == ValueType::Int => {
                self.compile_expr_as(builder, &binop.left, ValueType::Str)?;
                self.compile_expr_as(builder, &binop.right, ValueType::Int)?;
                builder.call(self.string_runtime.str_repeat);
                Ok(())
            }
            Operator::Mult if left_ty == ValueType::Int && right_ty == ValueType::Str => {
                self.compile_expr_as(builder, &binop.right, ValueType::Str)?;
                self.compile_expr_as(builder, &binop.left, ValueType::Int)?;
                builder.call(self.string_runtime.str_repeat);
                Ok(())
            }
            _ => Err(CompileError::Unsupported(format!(
                "unsupported string binary operator `{}`",
                binop.op
            ))),
        }
    }

    fn compile_subscript(
        &mut self,
        builder: &mut InstrSeqBuilder,
        subscript: &ruff_python_ast::ExprSubscript,
    ) -> Result<(), CompileError> {
        match self.expr_type(&subscript.value)? {
            ValueType::Str => match &*subscript.slice {
                Expr::Slice(slice) => {
                    if slice.step.is_some() {
                        return Err(CompileError::Unsupported(
                            "slice steps are not supported".to_owned(),
                        ));
                    }
                    self.compile_expr_as(builder, &subscript.value, ValueType::Str)?;
                    if let Some(lower) = slice.lower.as_deref() {
                        builder.i32_const(1);
                        self.compile_expr_as(builder, lower, ValueType::Int)?;
                    } else {
                        builder.i32_const(0).i64_const(0);
                    }
                    if let Some(upper) = slice.upper.as_deref() {
                        builder.i32_const(1);
                        self.compile_expr_as(builder, upper, ValueType::Int)?;
                    } else {
                        builder.i32_const(0).i64_const(0);
                    }
                    builder.call(self.string_runtime.str_slice);
                    Ok(())
                }
                index => {
                    self.compile_expr_as(builder, &subscript.value, ValueType::Str)?;
                    self.compile_expr_as(builder, index, ValueType::Int)?;
                    builder.call(self.string_runtime.str_char_at);
                    Ok(())
                }
            },
            other => Err(CompileError::Unsupported(format!(
                "unsupported subscript base type {:?}",
                other
            ))),
        }
    }

    fn emit_if_else(
        &mut self,
        builder: &mut InstrSeqBuilder,
        result: Option<ValType>,
        consequent: impl FnOnce(&mut Self, &mut InstrSeqBuilder) -> Result<(), CompileError>,
        alternative: impl FnOnce(&mut Self, &mut InstrSeqBuilder) -> Result<(), CompileError>,
    ) -> Result<(), CompileError> {
        let consequent = {
            let mut seq = builder.dangling_instr_seq(result);
            consequent(self, &mut seq)?;
            seq.id()
        };
        let alternative = {
            let mut seq = builder.dangling_instr_seq(result);
            alternative(self, &mut seq)?;
            seq.id()
        };
        builder.instr(IfElse {
            consequent,
            alternative,
        });
        Ok(())
    }

    fn binding(&self, name: &str) -> Result<Binding, CompileError> {
        if let Some(binding) = self.bindings.get(name) {
            return Ok(Binding::Local(*binding));
        }
        if let Some(binding) = self.globals.get(name) {
            return Ok(Binding::Global(*binding));
        }
        Err(CompileError::Unsupported(format!("unknown binding `{name}`")))
    }

    fn load_binding(&self, builder: &mut InstrSeqBuilder, binding: Binding) {
        match binding {
            Binding::Local(binding) => {
                builder.local_get(binding.id);
            }
            Binding::Global(binding) => {
                builder.instr(GlobalGet { global: binding.id });
            }
        }
    }

    fn store_binding(&self, builder: &mut InstrSeqBuilder, binding: Binding) {
        match binding {
            Binding::Local(binding) => {
                builder.local_set(binding.id);
            }
            Binding::Global(binding) => {
                builder.instr(GlobalSet { global: binding.id });
            }
        }
    }

    fn expr_type(&self, expr: &Expr) -> Result<ValueType, CompileError> {
        let ty = ExprRef::from(expr)
            .inferred_type(self.semantic)
            .ok_or_else(|| CompileError::Unsupported("missing inferred type".to_owned()))?;
        let display = simplify_type_display(&ty.display(self.db).to_string());
        match display.as_str() {
            "int" => Ok(ValueType::Int),
            "float" => Ok(ValueType::Float),
            "bool" => Ok(ValueType::Bool),
            "str" => Ok(ValueType::Str),
            "int | float" | "float | int" => Ok(ValueType::Float),
            "Any" | "Unknown" => self.fallback_expr_type(expr),
            other => Err(CompileError::Unsupported(format!(
                "unsupported inferred type `{other}`"
            ))),
        }
    }

    fn fallback_expr_type(&self, expr: &Expr) -> Result<ValueType, CompileError> {
        match expr {
            Expr::StringLiteral(_) => Ok(ValueType::Str),
            Expr::NumberLiteral(number) => match number.value {
                ruff_python_ast::Number::Int(_) => Ok(ValueType::Int),
                ruff_python_ast::Number::Float(_) => Ok(ValueType::Float),
                ruff_python_ast::Number::Complex { .. } => Err(CompileError::Unsupported(
                    "complex numbers are not supported".to_owned(),
                )),
            },
            Expr::BooleanLiteral(_) | Expr::Compare(_) | Expr::BoolOp(_) => Ok(ValueType::Bool),
            Expr::Name(name) => Ok(self.binding(name.id.as_str())?.ty()),
            Expr::UnaryOp(unary) => match unary.op {
                UnaryOp::Not => Ok(ValueType::Bool),
                UnaryOp::UAdd | UnaryOp::USub => self.expr_type(&unary.operand),
                UnaryOp::Invert => Err(CompileError::Unsupported(
                    "bitwise invert is not supported".to_owned(),
                )),
            },
            Expr::BinOp(binop) => self.fallback_binop_type(binop),
            Expr::Call(call) => self.fallback_call_type(call),
            Expr::Subscript(subscript) => match self.expr_type(&subscript.value)? {
                ValueType::Str => Ok(ValueType::Str),
                other => Err(CompileError::Unsupported(format!(
                    "unsupported subscript base type {:?}",
                    other
                ))),
            },
            other => Err(CompileError::Unsupported(format!(
                "unsupported inferred type `Any` for expression: {:?}",
                other
            ))),
        }
    }

    fn fallback_binop_type(
        &self,
        binop: &ruff_python_ast::ExprBinOp,
    ) -> Result<ValueType, CompileError> {
        let left = self.expr_type(&binop.left)?;
        let right = self.expr_type(&binop.right)?;
        if binop.op == Operator::Add && left == ValueType::Str && right == ValueType::Str {
            return Ok(ValueType::Str);
        }
        if binop.op == Operator::Mult
            && ((left == ValueType::Str && right == ValueType::Int)
                || (left == ValueType::Int && right == ValueType::Str))
        {
            return Ok(ValueType::Str);
        }
        match binop.op {
            Operator::Add | Operator::Sub | Operator::Mult | Operator::Mod => {
                common_numeric_type(left, right)
            }
            Operator::Div => Ok(ValueType::Float),
            Operator::FloorDiv => {
                if left == ValueType::Float || right == ValueType::Float {
                    Ok(ValueType::Float)
                } else {
                    Ok(ValueType::Int)
                }
            }
            Operator::Pow => {
                if left == ValueType::Float || right == ValueType::Float {
                    Ok(ValueType::Float)
                } else {
                    Ok(ValueType::Int)
                }
            }
            other => Err(CompileError::Unsupported(format!(
                "unsupported binary operator `{other}`"
            ))),
        }
    }

    fn fallback_call_type(&self, call: &ruff_python_ast::ExprCall) -> Result<ValueType, CompileError> {
        match &*call.func {
            Expr::Name(name) => {
                if let Some(signature) = self.signatures.get(name.id.as_str()) {
                    return Ok(signature.result);
                }
                fallback_builtin_call_type(
                    name.id.as_str(),
                    |expr| self.expr_type(expr),
                    &call.arguments.args,
                )
            }
            Expr::Attribute(attr) => fallback_attribute_call_type(
                attr,
                |expr| self.expr_type(expr),
                &call.arguments.args,
            ),
            other => Err(CompileError::Unsupported(format!(
                "unsupported callee expression: {:?}",
                other
            ))),
        }
    }

    fn binary_op(&self, op: Operator, ty: ValueType) -> Result<BinaryOp, CompileError> {
        match (op, ty) {
            (Operator::Add, ValueType::Int) => Ok(BinaryOp::I64Add),
            (Operator::Sub, ValueType::Int) => Ok(BinaryOp::I64Sub),
            (Operator::Mult, ValueType::Int) => Ok(BinaryOp::I64Mul),
            (Operator::Add, ValueType::Float) => Ok(BinaryOp::F64Add),
            (Operator::Sub, ValueType::Float) => Ok(BinaryOp::F64Sub),
            (Operator::Mult, ValueType::Float) => Ok(BinaryOp::F64Mul),
            (Operator::Div, ValueType::Float) => Ok(BinaryOp::F64Div),
            _ => Err(CompileError::Unsupported(format!(
                "unsupported binary operator `{}` for {:?}",
                op, ty
            ))),
        }
    }

    fn compare_op(&self, op: CmpOp, ty: ValueType) -> Result<BinaryOp, CompileError> {
        match (op, ty) {
            (CmpOp::Eq, ValueType::Int) => Ok(BinaryOp::I64Eq),
            (CmpOp::Eq, ValueType::Float) => Ok(BinaryOp::F64Eq),
            (CmpOp::Eq, ValueType::Bool) => Ok(BinaryOp::I32Eq),
            (CmpOp::Lt, ValueType::Int) => Ok(BinaryOp::I64LtS),
            (CmpOp::Lt, ValueType::Float) => Ok(BinaryOp::F64Lt),
            (CmpOp::LtE, ValueType::Int) => Ok(BinaryOp::I64LeS),
            (CmpOp::LtE, ValueType::Float) => Ok(BinaryOp::F64Le),
            (CmpOp::Gt, ValueType::Int) => Ok(BinaryOp::I64GtS),
            (CmpOp::Gt, ValueType::Float) => Ok(BinaryOp::F64Gt),
            (CmpOp::GtE, ValueType::Int) => Ok(BinaryOp::I64GeS),
            (CmpOp::GtE, ValueType::Float) => Ok(BinaryOp::F64Ge),
            _ => Err(CompileError::Unsupported(format!(
                "unsupported comparison `{}` for {:?}",
                op, ty
            ))),
        }
    }
}

fn validate_math_import(import: &StmtImport) -> Result<(), CompileError> {
    if import.is_lazy {
        return Err(CompileError::Unsupported(
            "lazy imports are not supported".to_owned(),
        ));
    }
    let [alias] = import.names.as_slice() else {
        return Err(CompileError::Unsupported(
            "only `import math` is currently supported".to_owned(),
        ));
    };
    if alias.name.as_str() != "math" || alias.asname.is_some() {
        return Err(CompileError::Unsupported(
            "only `import math` is currently supported".to_owned(),
        ));
    }
    Ok(())
}

fn name_target(expr: &Expr) -> Result<String, CompileError> {
    match expr {
        Expr::Name(name) => Ok(name.id.as_str().to_owned()),
        other => Err(CompileError::Unsupported(format!(
            "unsupported assignment target: {:?}",
            other
        ))),
    }
}

fn common_compare_type(left: ValueType, right: ValueType) -> Result<ValueType, CompileError> {
    match (left, right) {
        (lhs, rhs) if lhs == rhs => Ok(lhs),
        (ValueType::Int, ValueType::Float) | (ValueType::Float, ValueType::Int) => {
            Ok(ValueType::Float)
        }
        _ => Err(CompileError::Unsupported(format!(
            "unsupported comparison between {:?} and {:?}",
            left, right
        ))),
    }
}

fn common_numeric_type(left: ValueType, right: ValueType) -> Result<ValueType, CompileError> {
    match (left, right) {
        (lhs, rhs) if lhs == rhs && lhs != ValueType::Bool => Ok(lhs),
        (ValueType::Int, ValueType::Float) | (ValueType::Float, ValueType::Int) => {
            Ok(ValueType::Float)
        }
        _ => Err(CompileError::Unsupported(format!(
            "unsupported numeric combination {:?} and {:?}",
            left, right
        ))),
    }
}

fn fallback_builtin_call_type(
    callee: &str,
    expr_type: impl Fn(&Expr) -> Result<ValueType, CompileError>,
    args: &[Expr],
) -> Result<ValueType, CompileError> {
    match callee {
        "abs" => {
            let [arg] = args else {
                return Err(CompileError::Unsupported(
                    "`abs` expects exactly one argument".to_owned(),
                ));
            };
            expr_type(arg)
        }
        "len" => {
            let [arg] = args else {
                return Err(CompileError::Unsupported(
                    "`len` expects exactly one argument".to_owned(),
                ));
            };
            if expr_type(arg)? == ValueType::Str {
                Ok(ValueType::Int)
            } else {
                Err(CompileError::Unsupported(
                    "`len` is currently only supported for strings".to_owned(),
                ))
            }
        }
        "min" | "max" => {
            let [lhs, rhs] = args else {
                return Err(CompileError::Unsupported(
                    "`min`/`max` expect exactly two arguments".to_owned(),
                ));
            };
            common_numeric_type(expr_type(lhs)?, expr_type(rhs)?)
        }
        "round" => match args {
            [value] => match expr_type(value)? {
                ValueType::Int | ValueType::Float => Ok(ValueType::Int),
                ValueType::Bool => Err(CompileError::Unsupported(
                    "`round` on bool is not supported".to_owned(),
                )),
                ValueType::Str => Err(CompileError::Unsupported(
                    "`round` on str is not supported".to_owned(),
                )),
            },
            [value, digits] => {
                if expr_type(value)? == ValueType::Float && expr_type(digits)? == ValueType::Int {
                    Ok(ValueType::Float)
                } else {
                    Err(CompileError::Unsupported(
                        "`round(x, n)` currently expects `(float, int)`".to_owned(),
                    ))
                }
            }
            _ => Err(CompileError::Unsupported(
                "`round` expects one or two positional arguments".to_owned(),
            )),
        },
        "str" => {
            let [value] = args else {
                return Err(CompileError::Unsupported(
                    "`str` expects exactly one argument".to_owned(),
                ));
            };
            match expr_type(value)? {
                ValueType::Int | ValueType::Str => Ok(ValueType::Str),
                other => Err(CompileError::Unsupported(format!(
                    "`str` is not supported for {:?}",
                    other
                ))),
            }
        }
        other => Err(CompileError::Unsupported(format!(
            "unsupported builtin `{other}`"
        ))),
    }
}

fn fallback_attribute_call_type(
    attr: &ruff_python_ast::ExprAttribute,
    expr_type: impl Fn(&Expr) -> Result<ValueType, CompileError>,
    args: &[Expr],
) -> Result<ValueType, CompileError> {
    if let Expr::Name(module_name) = &*attr.value
        && module_name.id.as_str() == "math"
        && attr.attr.as_str() == "ceil"
    {
        let [arg] = args else {
            return Err(CompileError::Unsupported(
                "`math.ceil` expects exactly one argument".to_owned(),
            ));
        };
        let _ = arg;
        return Ok(ValueType::Int);
    }
    if args.is_empty() && expr_type(&attr.value)? == ValueType::Str {
        match attr.attr.as_str() {
            "upper" | "lower" => Ok(ValueType::Str),
            other => Err(CompileError::Unsupported(format!(
                "unsupported string method `{other}`"
            ))),
        }
    } else {
        Err(CompileError::Unsupported(format!(
            "unsupported attribute call `{:?}.{}(...)`",
            attr.value, attr.attr
        )))
    }
}

fn zero_const_expr(ty: ValueType, string_ty: TypeId) -> ConstExpr {
    match ty {
        ValueType::Int => ConstExpr::Value(WasmValue::I64(0)),
        ValueType::Float => ConstExpr::Value(WasmValue::F64(0.0)),
        ValueType::Bool => ConstExpr::Value(WasmValue::I32(0)),
        ValueType::Str => ConstExpr::RefNull(RefType {
            nullable: true,
            heap_type: HeapType::Concrete(string_ty),
        }),
    }
}

fn is_assignable_type(actual: ValueType, expected: ValueType) -> bool {
    actual == expected || (actual == ValueType::Int && expected == ValueType::Float)
}

fn ensure_assignable_type(
    actual: ValueType,
    expected: ValueType,
    context: &str,
) -> Result<(), CompileError> {
    if is_assignable_type(actual, expected) {
        Ok(())
    } else {
        Err(CompileError::Unsupported(format!(
            "{context} expects {:?}, got {:?}",
            expected, actual
        )))
    }
}

fn is_float_literal(expr: &Expr, expected: f64) -> bool {
    match expr {
        Expr::NumberLiteral(number) => match number.value {
            ruff_python_ast::Number::Float(value) => value == expected,
            _ => false,
        },
        _ => false,
    }
}

fn simplify_type_display(s: &str) -> String {
    if s == "LiteralString" {
        return "str".to_owned();
    }
    if let Some(inner) = s.strip_prefix("Literal[").and_then(|s| s.strip_suffix(']')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if !parts.is_empty() && parts.iter().all(|part| *part == "True" || *part == "False") {
            return "bool".to_owned();
        }
        if !parts.is_empty() && parts.iter().all(|part| part.parse::<i64>().is_ok()) {
            return "int".to_owned();
        }
        if !parts.is_empty() && parts.iter().all(|part| part.parse::<f64>().is_ok()) {
            return "float".to_owned();
        }
        if inner == "True" || inner == "False" {
            return "bool".to_owned();
        }
        if inner.parse::<i64>().is_ok() {
            return "int".to_owned();
        }
        if inner.parse::<f64>().is_ok() {
            return "float".to_owned();
        }
        if (inner.starts_with('\'') && inner.ends_with('\''))
            || (inner.starts_with('"') && inner.ends_with('"'))
        {
            return "str".to_owned();
        }
        return s.to_owned();
    }
    s.to_owned()
}

fn string_val_type(string_ty: TypeId) -> ValType {
    ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(string_ty),
    })
}

fn emit_empty_string(builder: &mut InstrSeqBuilder, str_ty: TypeId) {
    builder.i32_const(0).i32_const(0).array_new(str_ty);
}

fn collect_string_literals(suite: &[Stmt]) -> Vec<String> {
    let mut literals = BTreeSet::new();
    for stmt in suite {
        collect_stmt_string_literals(stmt, &mut literals);
    }
    literals.into_iter().collect()
}

fn collect_stmt_string_literals(stmt: &Stmt, literals: &mut BTreeSet<String>) {
    match stmt {
        Stmt::FunctionDef(function) => {
            for stmt in &function.body {
                collect_stmt_string_literals(stmt, literals);
            }
        }
        Stmt::Return(ret) => {
            if let Some(value) = ret.value.as_deref() {
                collect_expr_string_literals(value, literals);
            }
        }
        Stmt::Assign(assign) => {
            collect_expr_string_literals(&assign.value, literals);
        }
        Stmt::AnnAssign(assign) => {
            if let Some(value) = assign.value.as_deref() {
                collect_expr_string_literals(value, literals);
            }
        }
        Stmt::If(if_stmt) => {
            collect_expr_string_literals(&if_stmt.test, literals);
            for stmt in &if_stmt.body {
                collect_stmt_string_literals(stmt, literals);
            }
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_expr_string_literals(test, literals);
                }
                for stmt in &clause.body {
                    collect_stmt_string_literals(stmt, literals);
                }
            }
        }
        Stmt::Assert(assert_stmt) => {
            collect_expr_string_literals(&assert_stmt.test, literals);
        }
        Stmt::Expr(expr) if expr.value.is_string_literal_expr() => {}
        Stmt::Expr(expr) => {
            collect_expr_string_literals(&expr.value, literals);
        }
        Stmt::Pass(_) | Stmt::Import(_) => {}
        _ => {}
    }
}

fn collect_expr_string_literals(expr: &Expr, literals: &mut BTreeSet<String>) {
    match expr {
        Expr::StringLiteral(string) => {
            literals.insert(string.value.to_str().to_owned());
        }
        Expr::BinOp(binop) => {
            collect_expr_string_literals(&binop.left, literals);
            collect_expr_string_literals(&binop.right, literals);
        }
        Expr::UnaryOp(unary) => collect_expr_string_literals(&unary.operand, literals),
        Expr::BoolOp(bool_op) => {
            for value in &bool_op.values {
                collect_expr_string_literals(value, literals);
            }
        }
        Expr::Compare(compare) => {
            collect_expr_string_literals(&compare.left, literals);
            for comparator in &compare.comparators {
                collect_expr_string_literals(comparator, literals);
            }
        }
        Expr::Call(call) => {
            collect_expr_string_literals(&call.func, literals);
            for arg in call.arguments.args.iter() {
                collect_expr_string_literals(arg, literals);
            }
            for keyword in call.arguments.keywords.iter() {
                collect_expr_string_literals(&keyword.value, literals);
            }
        }
        Expr::Attribute(attr) => collect_expr_string_literals(&attr.value, literals),
        Expr::Subscript(subscript) => {
            collect_expr_string_literals(&subscript.value, literals);
            collect_expr_string_literals(&subscript.slice, literals);
        }
        Expr::Slice(slice) => {
            if let Some(lower) = slice.lower.as_deref() {
                collect_expr_string_literals(lower, literals);
            }
            if let Some(upper) = slice.upper.as_deref() {
                collect_expr_string_literals(upper, literals);
            }
            if let Some(step) = slice.step.as_deref() {
                collect_expr_string_literals(step, literals);
            }
        }
        Expr::Name(_) | Expr::BooleanLiteral(_) | Expr::NumberLiteral(_) => {}
        _ => {}
    }
}
