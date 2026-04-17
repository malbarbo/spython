//! Python type annotation → WASM type mapping.

use ruff_python_ast::Expr;
use wasm_encoder::ValType;

/// A resolved WASM type from a Python annotation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WasmType {
    I32,
    I64,   // Python int and bool (bool is int in Python)
    F64,   // Python float
    String,
    List(Box<WasmType>),
    Array(Box<WasmType>),
    Dict(Box<WasmType>, Box<WasmType>),
    Set(Box<WasmType>),
    Struct(String),
    None,
    Callable {
        params: Vec<WasmType>,
        ret: Box<WasmType>,
    },
}

impl WasmType {
    pub fn to_val_type(self) -> ValType {
        match self {
            WasmType::I32 => ValType::I32,
            WasmType::I64 => ValType::I64,
            WasmType::F64 => ValType::F64,
            WasmType::String => ValType::Ref(wasm_encoder::RefType {
                nullable: true,
                heap_type: wasm_encoder::HeapType::Abstract {
                    shared: false,
                    ty: wasm_encoder::AbstractHeapType::Array,
                },
            }),
            WasmType::Array(_) | WasmType::List(_) | WasmType::Dict(_, _) | WasmType::Set(_) => ValType::Ref(wasm_encoder::RefType {
                nullable: true,
                heap_type: wasm_encoder::HeapType::Abstract {
                    shared: false,
                    ty: wasm_encoder::AbstractHeapType::Array,
                },
            }),
            WasmType::Struct(_) => ValType::Ref(wasm_encoder::RefType {

                nullable: true,
                heap_type: wasm_encoder::HeapType::Abstract {
                    shared: false,
                    ty: wasm_encoder::AbstractHeapType::Struct,
                },
            }),
            WasmType::None => ValType::Ref(wasm_encoder::RefType {
                nullable: true,
                heap_type: wasm_encoder::HeapType::Abstract {
                    shared: false,
                    ty: wasm_encoder::AbstractHeapType::None,
                },
            }),
            WasmType::Callable { .. } => ValType::Ref(wasm_encoder::RefType {
                nullable: true,
                heap_type: wasm_encoder::HeapType::Abstract {
                    shared: false,
                    ty: wasm_encoder::AbstractHeapType::Func,
                },
            }),
        }
    }
}

/// Resolve a Python type annotation expression to a WASM type.
pub fn resolve_annotation(expr: &Expr) -> Result<WasmType, String> {
    match expr {
        Expr::Name(name) => match name.id.as_str() {
            "int" => Ok(WasmType::I64),
            "float" => Ok(WasmType::F64),
            "bool" => Ok(WasmType::I64),
            "str" => Ok(WasmType::String),
            "None" => Ok(WasmType::None),
            other => Ok(WasmType::Struct(other.to_string())),
        },
        Expr::Subscript(sub) => {
            if let Expr::Name(name) = sub.value.as_ref() {
                match name.id.as_str() {
                    "list" => {
                        return Ok(WasmType::List(Box::new(resolve_annotation(
                            sub.slice.as_ref(),
                        )?)));
                    }
                    "dict" => {
                        if let Expr::Tuple(tuple) = sub.slice.as_ref() {
                            if tuple.elts.len() == 2 {
                                return Ok(WasmType::Dict(
                                    Box::new(resolve_annotation(&tuple.elts[0])?),
                                    Box::new(resolve_annotation(&tuple.elts[1])?),
                                ));
                            }
                        }
                        return Err("dict requires two type arguments".to_string());
                    }
                    "set" => {
                        return Ok(WasmType::Set(Box::new(resolve_annotation(
                            sub.slice.as_ref(),
                        )?)));
                    }
                    "Callable" => {
                        // Callable[[param_types], return_type]
                        if let Expr::Tuple(tuple) = sub.slice.as_ref() {
                            if tuple.elts.len() == 2 {
                                let mut params = Vec::new();
                                if let Expr::List(param_list) = &tuple.elts[0] {
                                    for p in &param_list.elts {
                                        params.push(resolve_annotation(p)?);
                                    }
                                }
                                let ret = resolve_annotation(&tuple.elts[1])?;
                                return Ok(WasmType::Callable {
                                    params,
                                    ret: Box::new(ret),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(format!("Unsupported type subscript"))
        }
        Expr::NoneLiteral(_) => Ok(WasmType::None),
        _ => Err(format!("Unsupported type annotation")),
    }
}
