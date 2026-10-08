//! Execution tracing for step-by-step visualization.
//!
//! Runs a program once, capturing a snapshot at each trace event,
//! producing an immutable timeline for the frontend to navigate.

use std::cell::RefCell;
use std::collections::HashMap;

use rustpython::vm::builtins::{PyBytes, PyDict, PyFloat, PyInt, PyList, PyStr};
use rustpython::vm::{self, AsObject, PyObjectRef};
use serde::{Deserialize, Serialize};

use crate::checker::Level;
use crate::output;

const USER_FILE: &str = "user.py";
const SINGLE_FILE_MSG: &str =
    "Step-by-step mode only supports single-file programs. Remove user-file imports to use it.";

// === Data types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub steps: Vec<TraceStep>,
    pub truncated: bool,
    pub exit_kind: ExitKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TraceErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitKind {
    Normal,
    Error,
    StepLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub line: u32,
    pub event: String,
    pub stdout_delta: String,
    pub stderr_delta: String,
    pub stack: Vec<StackFrameView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heap: Vec<HeapObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrameView {
    pub name: String,
    pub line: u32,
    pub locals: Vec<NamedValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedValue {
    pub name: String,
    pub value: TraceValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TraceValue {
    Ref { object_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HeapValue {
    None,
    Bool {
        value: bool,
    },
    Int {
        value: String,
    },
    Float {
        value: String,
    },
    Str {
        value: String,
    },
    Bytes {
        value: String,
    },
    List {
        items: Vec<TraceValue>,
    },
    Dict {
        entries: Vec<DictEntry>,
    },
    Dataclass {
        class_name: String,
        fields: Vec<NamedValue>,
    },
    Object {
        class_name: String,
        fields: Vec<NamedValue>,
    },
    Function {
        name: String,
    },
    Class {
        name: String,
    },
    Opaque {
        class_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapObject {
    pub object_id: u64,
    #[serde(flatten)]
    pub value: HeapValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    pub key: TraceValue,
    pub value: TraceValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceErrorInfo {
    pub exception_type: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

// === Collector ===

struct PendingStep {
    step: TraceStep,
    heap_index_by_id: HashMap<u64, usize>,
}

impl PendingStep {
    fn new(line: u32, event: String, stdout_delta: String, stderr_delta: String) -> Self {
        Self {
            step: TraceStep {
                line,
                event,
                stdout_delta,
                stderr_delta,
                stack: Vec::new(),
                heap: Vec::new(),
            },
            heap_index_by_id: HashMap::new(),
        }
    }
}

struct TraceCollector {
    steps: Vec<TraceStep>,
    current_step: Option<PendingStep>,
    stdout_mark: usize,
    stderr_mark: usize,
    max_steps: usize,
    truncated: bool,
    trace_id_by_runtime_id: HashMap<u64, u64>,
    next_object_id: u64,
}

impl TraceCollector {
    fn new(max_steps: usize) -> Self {
        Self {
            steps: Vec::new(),
            current_step: None,
            stdout_mark: 0,
            stderr_mark: 0,
            max_steps,
            truncated: false,
            trace_id_by_runtime_id: HashMap::new(),
            next_object_id: 1,
        }
    }

    fn is_full(&self) -> bool {
        self.steps.len() + usize::from(self.current_step.is_some()) >= self.max_steps
    }
}

thread_local! {
    static COLLECTOR: RefCell<Option<TraceCollector>> = const { RefCell::new(None) };
}

// === Native functions exposed to the Python tracer ===

/// Called by the Python tracer for each trace event.
/// Returns: 1 = recorded, 0 = skip (nested event), -1 = step limit reached.
fn on_event(event: String, line: usize) -> i32 {
    COLLECTOR.with(|c| {
        let mut opt = c.borrow_mut();
        let collector = match opt.as_mut() {
            Some(c) => c,
            None => return 0,
        };
        // Skip nested events while a snapshot is being built.
        if collector.current_step.is_some() {
            return 0;
        }
        if collector.is_full() {
            collector.truncated = true;
            return -1;
        }

        let stdout_delta = output::take_stdout_since(collector.stdout_mark);
        let stderr_delta = output::take_stderr_since(collector.stderr_mark);
        collector.stdout_mark += stdout_delta.len();
        collector.stderr_mark += stderr_delta.len();

        collector.current_step = Some(PendingStep::new(
            line as u32,
            event,
            stdout_delta,
            stderr_delta,
        ));

        1
    })
}

/// Add a stack frame to the current step.
fn push_frame(name: String, line: usize) {
    COLLECTOR.with(|c| {
        if let Some(collector) = c.borrow_mut().as_mut()
            && let Some(step) = &mut collector.current_step
        {
            step.step.stack.push(StackFrameView {
                name,
                line: line as u32,
                locals: Vec::new(),
            });
        }
    });
}

/// Add a local variable to the last frame in the current step.
fn push_local(key: String, value: PyObjectRef, vm: &vm::VirtualMachine) {
    COLLECTOR.with(|c| {
        if let Some(collector) = c.borrow_mut().as_mut() {
            if should_skip_local(&key, &value) {
                return;
            }
            let TraceCollector {
                current_step,
                trace_id_by_runtime_id,
                next_object_id,
                ..
            } = collector;
            let Some(step) = current_step.as_mut() else {
                return;
            };
            let value = snapshot_value(step, trace_id_by_runtime_id, next_object_id, value, vm, 0);
            let Some(frame) = step.step.stack.last_mut() else {
                return;
            };
            frame.locals.push(NamedValue { name: key, value });
        }
    });
}

/// Finalize the current step and add it to the timeline.
fn commit() {
    COLLECTOR.with(|c| {
        if let Some(collector) = c.borrow_mut().as_mut()
            && let Some(mut pending) = collector.current_step.take()
        {
            // Stack was built top-down (current frame first).
            // Reverse so module frame is first (bottom of stack).
            pending.step.stack.reverse();
            collector.steps.push(pending.step);
        }
    });
}

fn should_skip_local(key: &str, value: &PyObjectRef) -> bool {
    if key.starts_with('_') {
        return true;
    }
    let class_name = value.class().name();
    matches!(&*class_name, "module" | "builtin_function_or_method")
}

fn snapshot_value(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    vm: &vm::VirtualMachine,
    depth: usize,
) -> TraceValue {
    if vm.is_none(&value) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::None,
        );
    }
    if value.is(&vm.ctx.true_value) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Bool { value: true },
        );
    }
    if value.is(&vm.ctx.false_value) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Bool { value: false },
        );
    }
    if let Ok(int_value) = value.clone().downcast_exact::<PyInt>(vm) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Int {
                value: int_value.to_str_radix_10(),
            },
        );
    }
    if let Ok(float_value) = value.clone().downcast_exact::<PyFloat>(vm) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Float {
                value: float_value.to_f64().to_string(),
            },
        );
    }
    if let Ok(str_value) = value.clone().downcast_exact::<PyStr>(vm) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Str {
                value: str_value.to_string(),
            },
        );
    }
    if let Ok(bytes_value) = value.clone().downcast_exact::<PyBytes>(vm) {
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Bytes {
                value: format!("{:?}", bytes_value.as_bytes()),
            },
        );
    }
    if let Ok(list_value) = value.clone().downcast_exact::<PyList>(vm) {
        return snapshot_list(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            list_value,
            vm,
            depth,
        );
    }
    if let Ok(dict_value) = value.clone().downcast_exact::<PyDict>(vm) {
        return snapshot_dict(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            dict_value,
            vm,
            depth,
        );
    }

    let class_name = value.class().name().to_string();
    if class_name == "function" {
        let name =
            named_object(&value, "__name__", vm).unwrap_or_else(|| "<anonymous>".to_string());
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Function { name },
        );
    }
    if class_name == "type" {
        let name = named_object(&value, "__name__", vm).unwrap_or_else(|| class_name.clone());
        return snapshot_leaf_value(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            HeapValue::Class { name },
        );
    }

    if dataclass_field_names(value.class(), vm).is_some() {
        return snapshot_dataclass(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            vm,
            depth + 1,
        );
    }
    if value.dict().is_some() {
        return snapshot_object(
            pending,
            trace_id_by_runtime_id,
            next_object_id,
            value,
            vm,
            depth + 1,
        );
    }

    snapshot_leaf_value(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        value,
        HeapValue::Opaque { class_name },
    )
}

fn snapshot_leaf_value(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    heap_value: HeapValue,
) -> TraceValue {
    let runtime_id = value.get_id() as u64;
    let (object_id, _heap_index, _is_new) = ensure_heap_slot(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        runtime_id,
        heap_value,
    );
    TraceValue::Ref { object_id }
}

fn ensure_heap_slot(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    runtime_id: u64,
    initial_value: HeapValue,
) -> (u64, usize, bool) {
    if let Some(&object_id) = trace_id_by_runtime_id.get(&runtime_id) {
        if let Some(&heap_index) = pending.heap_index_by_id.get(&object_id) {
            return (object_id, heap_index, false);
        }

        let heap_index = pending.step.heap.len();
        pending.step.heap.push(HeapObject {
            object_id,
            value: initial_value,
        });
        pending.heap_index_by_id.insert(object_id, heap_index);
        return (object_id, heap_index, true);
    }

    let object_id = *next_object_id;
    *next_object_id += 1;
    let heap_index = pending.step.heap.len();
    pending.step.heap.push(HeapObject {
        object_id,
        value: initial_value,
    });
    trace_id_by_runtime_id.insert(runtime_id, object_id);
    pending.heap_index_by_id.insert(object_id, heap_index);
    (object_id, heap_index, true)
}

fn snapshot_list(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    list_value: vm::PyRefExact<PyList>,
    vm: &vm::VirtualMachine,
    depth: usize,
) -> TraceValue {
    let runtime_id = value.get_id() as u64;
    let (object_id, heap_index, is_new) = ensure_heap_slot(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        runtime_id,
        HeapValue::List { items: Vec::new() },
    );
    if !is_new {
        return TraceValue::Ref { object_id };
    }

    let elements: Vec<_> = list_value.borrow_vec().iter().cloned().collect();
    let items = elements
        .into_iter()
        .map(|item| {
            snapshot_value(
                pending,
                trace_id_by_runtime_id,
                next_object_id,
                item,
                vm,
                depth + 1,
            )
        })
        .collect();
    if let Some(heap_object) = pending.step.heap.get_mut(heap_index) {
        heap_object.value = HeapValue::List { items };
    }

    TraceValue::Ref { object_id }
}

fn snapshot_dict(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    dict_value: vm::PyRefExact<PyDict>,
    vm: &vm::VirtualMachine,
    depth: usize,
) -> TraceValue {
    let runtime_id = value.get_id() as u64;
    let (object_id, heap_index, is_new) = ensure_heap_slot(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        runtime_id,
        HeapValue::Dict {
            entries: Vec::new(),
        },
    );
    if !is_new {
        return TraceValue::Ref { object_id };
    }

    let entries = dict_value
        .items_vec()
        .into_iter()
        .map(|(key, item_value)| DictEntry {
            key: snapshot_value(
                pending,
                trace_id_by_runtime_id,
                next_object_id,
                key,
                vm,
                depth + 1,
            ),
            value: snapshot_value(
                pending,
                trace_id_by_runtime_id,
                next_object_id,
                item_value,
                vm,
                depth + 1,
            ),
        })
        .collect();
    if let Some(heap_object) = pending.step.heap.get_mut(heap_index) {
        heap_object.value = HeapValue::Dict { entries };
    }

    TraceValue::Ref { object_id }
}

fn snapshot_dataclass(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    vm: &vm::VirtualMachine,
    depth: usize,
) -> TraceValue {
    let class_name = value.class().name().to_string();
    let runtime_id = value.get_id() as u64;
    let (object_id, heap_index, is_new) = ensure_heap_slot(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        runtime_id,
        HeapValue::Dataclass {
            class_name: class_name.clone(),
            fields: Vec::new(),
        },
    );
    if !is_new {
        return TraceValue::Ref { object_id };
    }

    let field_names = dataclass_field_names(value.class(), vm).unwrap_or_default();
    let object_dict = value.dict();
    let cls = value.class().to_owned();

    let mut fields = Vec::new();
    for field_name in field_names {
        let field_value = if let Some(dict) = &object_dict {
            match dict.get_item_opt(vm.ctx.intern_str(field_name.as_str()), vm) {
                Ok(Some(field_value)) => Some(field_value),
                _ => None,
            }
        } else {
            read_slotted_field(&value, &cls, field_name.as_str(), vm)
        };
        let Some(field_value) = field_value else {
            continue;
        };
        fields.push(NamedValue {
            name: field_name,
            value: snapshot_value(
                pending,
                trace_id_by_runtime_id,
                next_object_id,
                field_value,
                vm,
                depth,
            ),
        });
    }

    if let Some(heap_object) = pending.step.heap.get_mut(heap_index) {
        heap_object.value = HeapValue::Dataclass { class_name, fields };
    }

    TraceValue::Ref { object_id }
}

fn dataclass_field_names(
    cls: &vm::Py<vm::builtins::PyType>,
    vm: &vm::VirtualMachine,
) -> Option<Vec<String>> {
    let fields = cls.get_attr(vm.ctx.intern_str("__dataclass_fields__"))?;
    let fields = fields.downcast_exact::<PyDict>(vm).ok()?;
    Some(
        fields
            .items_vec()
            .into_iter()
            .filter_map(|(key, _)| {
                key.downcast_exact::<PyStr>(vm)
                    .ok()
                    .map(|key| key.to_string())
            })
            .collect(),
    )
}

fn read_slotted_field(
    value: &PyObjectRef,
    cls: &vm::Py<vm::builtins::PyType>,
    field_name: &str,
    vm: &vm::VirtualMachine,
) -> Option<PyObjectRef> {
    let descriptor = cls.get_attr(vm.ctx.intern_str(field_name))?;
    if !descriptor.class().is(vm.ctx.types.member_descriptor_type) {
        return None;
    }
    let result = vm.call_get_descriptor_specific(
        descriptor.as_object(),
        Some(value.clone()),
        Some(cls.to_owned().into()),
    )?;
    result.ok()
}

fn snapshot_object(
    pending: &mut PendingStep,
    trace_id_by_runtime_id: &mut HashMap<u64, u64>,
    next_object_id: &mut u64,
    value: PyObjectRef,
    vm: &vm::VirtualMachine,
    depth: usize,
) -> TraceValue {
    let class_name = value.class().name().to_string();
    let runtime_id = value.get_id() as u64;
    let (object_id, heap_index, is_new) = ensure_heap_slot(
        pending,
        trace_id_by_runtime_id,
        next_object_id,
        runtime_id,
        HeapValue::Object {
            class_name: class_name.clone(),
            fields: Vec::new(),
        },
    );
    if !is_new {
        return TraceValue::Ref { object_id };
    }

    let Some(dict) = value.dict() else {
        return TraceValue::Ref { object_id };
    };
    let mut fields: Vec<_> = dict
        .items_vec()
        .into_iter()
        .filter_map(|(key, item_value)| {
            let key = key.downcast_exact::<PyStr>(vm).ok()?;
            Some((key.to_string(), item_value))
        })
        .collect();
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    let fields = fields
        .into_iter()
        .map(|(name, field_value)| NamedValue {
            name,
            value: snapshot_value(
                pending,
                trace_id_by_runtime_id,
                next_object_id,
                field_value,
                vm,
                depth,
            ),
        })
        .collect();

    if let Some(heap_object) = pending.step.heap.get_mut(heap_index) {
        heap_object.value = HeapValue::Object { class_name, fields };
    }

    TraceValue::Ref { object_id }
}

fn named_object(
    value: &PyObjectRef,
    attr: &'static str,
    vm: &vm::VirtualMachine,
) -> Option<String> {
    value
        .get_attr(vm.ctx.intern_str(attr), vm)
        .ok()?
        .downcast_exact::<PyStr>(vm)
        .ok()
        .map(|value| value.to_string())
}

// === Trace module registration ===

fn register_trace_module(vm: &vm::VirtualMachine) {
    let dict = vm.ctx.new_dict();

    dict.set_item(
        "_on_event",
        vm.new_function("_on_event", on_event).into(),
        vm,
    )
    .unwrap();
    dict.set_item(
        "_push_frame",
        vm.new_function("_push_frame", push_frame).into(),
        vm,
    )
    .unwrap();
    dict.set_item(
        "_push_local",
        vm.new_function("_push_local", push_local).into(),
        vm,
    )
    .unwrap();
    dict.set_item("_commit", vm.new_function("_commit", commit).into(), vm)
        .unwrap();

    let module = vm.new_module("_spython_trace", dict, None);
    vm.sys_module
        .get_attr("modules", vm)
        .expect("sys.modules should exist")
        .set_item("_spython_trace", module.into(), vm)
        .expect("should be able to add to sys.modules");
}

// === Python tracer code ===

fn tracer_python_code() -> String {
    format!(
        r#"import sys
from _spython_trace import _on_event, _push_frame, _push_local, _commit

def __spython_tracer__(frame, event, arg):
    if frame.f_code.co_filename != '{USER_FILE}':
        return None
    result = _on_event(event, frame.f_lineno)
    if result < 0:
        sys.settrace(None)
        return None
    if result == 0:
        return __spython_tracer__
    f = frame
    while f is not None:
        if f.f_code.co_filename == '{USER_FILE}':
            fn_name = f.f_code.co_name
            ns = f.f_globals if fn_name == '<module>' else f.f_locals
            _push_frame(fn_name, f.f_lineno)
            for k in sorted(ns):
                _push_local(k, ns[k])
        f = f.f_back
    _commit()
    return __spython_tracer__

sys.settrace(__spython_tracer__)
"#
    )
}

// === Main entry point ===

/// Execute `source` with tracing and return a JSON-serialized [`ExecutionTrace`].
pub fn trace_source(source: &str, level: Level, max_steps: usize) -> String {
    // Type check first
    match crate::type_check_source(source, level) {
        Ok(Some(te)) => {
            crate::print_type_errors(&te.db, &te.diagnostics, true);
        }
        Err(te) => {
            // Check for user file imports (unresolved-import not from spython)
            let has_user_import = te.diagnostics.iter().any(|d| {
                d.id().as_str() == "unresolved-import" && !d.primary_message().contains("spython")
            });
            if has_user_import {
                eprintln!("{SINGLE_FILE_MSG}");
                return error_trace("ImportError", SINGLE_FILE_MSG);
            }
            crate::print_type_errors(&te.db, &te.diagnostics, true);
            return error_trace("TypeError", "Type checking failed");
        }
        Ok(None) => {}
    }

    trace_source_prechecked(source, max_steps)
}

/// Execute `source` assuming type checking already happened.
pub fn trace_source_prechecked(source: &str, max_steps: usize) -> String {
    // Initialize collector
    COLLECTOR.with(|c| {
        *c.borrow_mut() = Some(TraceCollector::new(max_steps));
    });

    let interp = crate::new_interpreter();
    let source_owned = source.to_owned();

    let (exit_kind, error) = interp.enter(|vm| {
        let scope = vm
            .new_scope_with_main()
            .expect("creating main scope should not fail");
        crate::register_ffi_module(vm);
        crate::install_capture_writers(vm);
        output::clear_buffers();

        // Register trace native module
        register_trace_module(vm);

        // Install Python tracer in a separate scope (avoids polluting user namespace)
        let setup_scope = vm.new_scope_with_builtins();
        if let Err(exc) = vm.run_string(
            setup_scope,
            &tracer_python_code(),
            "<trace-setup>".to_owned(),
        ) {
            vm.print_exception(exc);
            return (
                ExitKind::Error,
                Some(TraceErrorInfo {
                    exception_type: "InternalError".to_string(),
                    message: "Failed to install tracer".to_string(),
                    line: None,
                }),
            );
        }

        // Execute user code
        let result = vm
            .compile(
                &source_owned,
                vm::compiler::Mode::Exec,
                USER_FILE.to_owned(),
            )
            .map_err(|err| vm.new_syntax_error(&err, Some(&source_owned)))
            .and_then(|code_obj| vm.run_code_obj(code_obj, scope));

        // Disable tracer
        let teardown_scope = vm.new_scope_with_builtins();
        let _ = vm.run_string(
            teardown_scope,
            "import sys; sys.settrace(None)",
            "<teardown>".to_owned(),
        );

        match result {
            Ok(_) => {
                capture_remaining_output("finished");
                let truncated =
                    COLLECTOR.with(|c| c.borrow().as_ref().is_some_and(|c| c.truncated));
                let exit_kind = if truncated {
                    ExitKind::StepLimit
                } else {
                    ExitKind::Normal
                };
                (exit_kind, None)
            }
            Err(exc) => {
                let exception_type = exc.class().name().to_string();
                let message = exc
                    .as_object()
                    .str(vm)
                    .map(|s: vm::builtins::PyStrRef| s.to_string())
                    .unwrap_or_default();
                capture_remaining_output("exception");
                let truncated =
                    COLLECTOR.with(|c| c.borrow().as_ref().is_some_and(|c| c.truncated));
                let exit_kind = if truncated {
                    ExitKind::StepLimit
                } else {
                    ExitKind::Error
                };
                (
                    exit_kind,
                    Some(TraceErrorInfo {
                        exception_type,
                        message,
                        line: None,
                    }),
                )
            }
        }
    });

    // Extract trace and serialize
    let trace = COLLECTOR.with(|c| {
        let collector = c
            .borrow_mut()
            .take()
            .unwrap_or_else(|| TraceCollector::new(0));
        ExecutionTrace {
            steps: collector.steps,
            truncated: collector.truncated,
            exit_kind,
            error,
        }
    });

    serde_json::to_string(&trace).unwrap()
}

fn error_trace(exception_type: &str, message: &str) -> String {
    serde_json::to_string(&ExecutionTrace {
        steps: Vec::new(),
        truncated: false,
        exit_kind: ExitKind::Error,
        error: Some(TraceErrorInfo {
            exception_type: exception_type.to_string(),
            message: message.to_string(),
            line: None,
        }),
    })
    .unwrap()
}

fn capture_remaining_output(event: &str) {
    COLLECTOR.with(|c| {
        if let Some(collector) = c.borrow_mut().as_mut() {
            let stdout_delta = output::take_stdout_since(collector.stdout_mark);
            let stderr_delta = output::take_stderr_since(collector.stderr_mark);
            collector.stdout_mark += stdout_delta.len();
            collector.stderr_mark += stderr_delta.len();
            if stdout_delta.is_empty() && stderr_delta.is_empty() {
                return;
            }
            if collector.is_full() {
                collector.truncated = true;
                return;
            }
            let last_line = collector.steps.last().map(|s| s.line).unwrap_or(0);
            collector.steps.push(TraceStep {
                line: last_line,
                event: event.to_string(),
                stdout_delta,
                stderr_delta,
                stack: Vec::new(),
                heap: Vec::new(),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_trace(source: &str) -> ExecutionTrace {
        let json = trace_source(source, Level::Full, 1000);
        serde_json::from_str(&json).unwrap()
    }

    fn module_frame(step: &TraceStep) -> &StackFrameView {
        step.stack
            .iter()
            .find(|frame| frame.name == "<module>")
            .expect("should have <module> frame")
    }

    fn local_value<'a>(frame: &'a StackFrameView, name: &str) -> &'a TraceValue {
        &frame
            .locals
            .iter()
            .find(|binding| binding.name == name)
            .unwrap_or_else(|| panic!("missing local '{name}'"))
            .value
    }

    fn ref_id(value: &TraceValue) -> u64 {
        match value {
            TraceValue::Ref { object_id } => *object_id,
        }
    }

    fn heap_value(step: &TraceStep, object_id: u64) -> &HeapValue {
        &step
            .heap
            .iter()
            .find(|heap_object| heap_object.object_id == object_id)
            .unwrap_or_else(|| panic!("missing heap object #{object_id}"))
            .value
    }

    fn field_value<'a>(fields: &'a [NamedValue], name: &str) -> &'a TraceValue {
        &fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("missing field '{name}'"))
            .value
    }

    #[test]
    fn simple_assignment_generates_steps() {
        let trace = parse_trace("x: int = 1\n");
        assert!(!trace.steps.is_empty(), "should have at least one step");
        assert!(matches!(trace.exit_kind, ExitKind::Normal));
        assert!(!trace.truncated);
        assert!(trace.error.is_none());
    }

    #[test]
    fn print_appears_in_stdout_delta() {
        let trace = parse_trace("x: int = 42\nprint(x)\n");
        let has_output = trace.steps.iter().any(|s| s.stdout_delta.contains("42"));
        assert!(has_output, "print output should appear in stdout_delta");
    }

    #[test]
    fn function_call_shows_stack() {
        let trace = parse_trace("def foo(x: int) -> int:\n    return x + 1\ny: int = foo(10)\n");
        let has_nested_stack = trace.steps.iter().any(|s| s.stack.len() > 1);
        assert!(
            has_nested_stack,
            "function call should show nested stack frames"
        );
    }

    #[test]
    fn runtime_error_produces_partial_trace() {
        let trace = parse_trace("x: int = 1\ny: int = 1 // 0\n");
        assert!(matches!(trace.exit_kind, ExitKind::Error));
        assert!(trace.error.is_some());
        let err = trace.error.unwrap();
        assert_eq!(err.exception_type, "ZeroDivisionError");
    }

    #[test]
    fn step_limit_truncates() {
        let json = trace_source(
            "x: int = 0\nwhile x < 100:\n    x = x + 1\n",
            Level::Full,
            10,
        );
        let trace: ExecutionTrace = serde_json::from_str(&json).unwrap();
        assert!(trace.truncated);
        assert!(matches!(trace.exit_kind, ExitKind::StepLimit));
        assert!(trace.steps.len() <= 10);
    }

    #[test]
    fn zero_step_limit_does_not_emit_extra_output_step() {
        let json = trace_source("print(1)\n", Level::Full, 0);
        let trace: ExecutionTrace = serde_json::from_str(&json).unwrap();
        assert!(trace.truncated);
        assert!(matches!(trace.exit_kind, ExitKind::StepLimit));
        assert!(trace.steps.is_empty());
    }

    #[test]
    fn locals_appear_in_stack() {
        let trace = parse_trace("x: int = 42\ny: str = 'hello'\n");
        let last = trace.steps.last().unwrap();
        let locals = &module_frame(last).locals;
        assert!(locals.iter().any(|binding| {
            binding.name == "x" && matches!(&binding.value, TraceValue::Ref { .. })
        }));
        let x_id = ref_id(local_value(module_frame(last), "x"));
        assert!(matches!(heap_value(last, x_id), HeapValue::Int { value } if value == "42"));
    }

    #[test]
    fn heap_ids_start_at_one() {
        let trace = parse_trace("x: int = 42\n");
        let last = trace.steps.last().unwrap();
        let x_id = ref_id(local_value(module_frame(last), "x"));
        assert_eq!(x_id, 1);
        assert_eq!(last.heap.first().map(|object| object.object_id), Some(1));
    }

    #[test]
    fn aliasing_is_preserved_for_lists() {
        let trace = parse_trace("xs: list[int] = [1]\na: list[int] = xs\nb: list[int] = xs\n");
        let last = trace.steps.last().unwrap();
        let frame = module_frame(last);

        let xs_id = ref_id(local_value(frame, "xs"));
        let a_id = ref_id(local_value(frame, "a"));
        let b_id = ref_id(local_value(frame, "b"));
        assert_eq!(xs_id, a_id);
        assert_eq!(a_id, b_id);

        let HeapValue::List { items } = heap_value(last, xs_id) else {
            panic!("shared object should be a list");
        };
        assert_eq!(items.len(), 1);
        let item_id = ref_id(&items[0]);
        assert!(matches!(heap_value(last, item_id), HeapValue::Int { value } if value == "1"));
    }

    #[test]
    fn object_ids_are_stable_across_steps() {
        let trace = parse_trace("xs: list[int] = [1]\nxs.append(2)\n");
        let ids: Vec<_> = trace
            .steps
            .iter()
            .filter_map(|step| {
                let frame = step.stack.iter().find(|frame| frame.name == "<module>")?;
                let binding = frame.locals.iter().find(|binding| binding.name == "xs")?;
                Some(ref_id(&binding.value))
            })
            .collect();

        assert!(ids.len() >= 2, "xs should appear in multiple steps");
        assert!(ids.windows(2).all(|window| window[0] == window[1]));
    }

    #[test]
    fn dataclass_locals_are_structured() {
        let trace = parse_trace(
            r#"from dataclasses import dataclass
@dataclass
class Point:
    x: int
    y: int
p: Point = Point(1, 2)
        "#,
        );
        let last = trace.steps.last().unwrap();
        let frame = module_frame(last);
        let p_id = ref_id(local_value(frame, "p"));
        let HeapValue::Dataclass { class_name, fields } = heap_value(last, p_id) else {
            panic!("p should point to a structured dataclass");
        };
        assert_eq!(class_name, "Point");
        let x_id = ref_id(field_value(fields, "x"));
        let y_id = ref_id(field_value(fields, "y"));
        assert!(matches!(heap_value(last, x_id), HeapValue::Int { value } if value == "1"));
        assert!(matches!(heap_value(last, y_id), HeapValue::Int { value } if value == "2"));
    }

    #[test]
    fn slotted_dataclass_locals_are_structured() {
        let trace = parse_trace(
            r#"from dataclasses import dataclass
@dataclass(slots=True)
class Point:
    x: int
    y: int
p: Point = Point(3, 4)
        "#,
        );
        let last = trace.steps.last().unwrap();
        let frame = module_frame(last);
        let p_id = ref_id(local_value(frame, "p"));
        let HeapValue::Dataclass { class_name, fields } = heap_value(last, p_id) else {
            panic!("p should point to a structured dataclass");
        };
        assert_eq!(class_name, "Point");
        let x_id = ref_id(field_value(fields, "x"));
        let y_id = ref_id(field_value(fields, "y"));
        assert!(matches!(heap_value(last, x_id), HeapValue::Int { value } if value == "3"));
        assert!(matches!(heap_value(last, y_id), HeapValue::Int { value } if value == "4"));
    }

    #[test]
    fn import_user_file_rejected() {
        let trace = parse_trace("import my_utils\nx: int = 1\n");
        assert!(matches!(trace.exit_kind, ExitKind::Error));
        let err = trace.error.as_ref().unwrap();
        assert_eq!(err.exception_type, "ImportError");
        assert!(err.message.contains("single-file"));
    }

    #[test]
    fn import_spython_allowed() {
        let trace = parse_trace("from spython.image import *\nx: int = 1\n");
        // Should not fail with ImportError (may fail for other reasons in test env)
        if let Some(err) = &trace.error {
            assert_ne!(err.exception_type, "ImportError");
        }
    }
}
