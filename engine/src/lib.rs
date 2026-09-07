pub mod output;

pub mod check;
pub mod checker;
pub mod completion;
pub mod doctests;
pub mod error;
pub mod lints;
pub mod panic;
pub mod parser;
pub mod repl;
pub mod run;
pub mod spython_lib;
pub mod wasm_ffi;

pub use check::{
    TypeErrors, Warnings, annotation_check, check_file_doctests, check_source,
    collect_import_files, make_project_metadata, modules_beside, print_type_errors,
};
pub(crate) use check::{infer_expression_type, type_check_repl_input};
pub use checker::Level;
pub use error::{SpythonError, read_file, show_error};
pub use repl::{
    REPL_ERROR, REPL_OK, REPL_QUIT, ReplOutcome, ReplSession, ReplState, repl_new, repl_run,
};
pub use run::{DOCTEST_RUNNER, new_interpreter, run_source};

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use ruff_python_formatter::{DocstringCode, PyFormatOptions, QuoteStyle, format_module_source};
use rustpython::vm;
pub use ty_project::ProjectDatabase;

/// Package version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Libraries bundled in this build (set by build.rs).
pub const LIBS_VERSION: &str = env!("LIBS_VERSION");
/// Build date in YYYY-MM-DD format (set by build.rs).
pub const BUILD_DATE: &str = env!("BUILD_DATE");
/// Short git commit hash (set by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");
/// Full version string: "0.1.0 (rustpython ..., 2026-04-01, abc1234)".
pub const LONG_VERSION: &str = env!("LONG_VERSION");

/// Format Python source with ruff's formatter.
/// Returns `Err(message)` if the source cannot be parsed.
pub fn format_source(source: &str) -> Result<String, String> {
    let options = PyFormatOptions::default()
        .with_docstring_code(DocstringCode::Enabled)
        .with_quote_style(QuoteStyle::Single);
    format_module_source(source, options)
        .map(|f| f.into_code())
        .map_err(|e| e.to_string())
}

/// Set by the `__test_mark_atexit_ran` function that `register_ffi_module`
/// adds under test, so [`crate::repl`] can see that dropping a session ran
/// Python's atexit handlers.
#[cfg(test)]
static REPL_ATEXIT_RAN: AtomicBool = AtomicBool::new(false);

/// Register the `_spython_ffi` module with native `show_svg` and `get_key_event` functions.
fn register_ffi_module(vm: &vm::VirtualMachine) {
    use rustpython::vm::PyObjectRef;

    let show_svg_fn = vm.new_function(
        "show_svg",
        |svg: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            wasm_ffi::show_svg(&svg);
            Ok(vm.ctx.none())
        },
    );

    let get_key_event_fn =
        vm.new_function("get_key_event", |vm: &vm::VirtualMachine| -> PyObjectRef {
            match wasm_ffi::poll_key_event() {
                None => vm.ctx.none(),
                Some((event_type, key, mods)) => {
                    let elements = vec![
                        vm.ctx.new_int(event_type).into(),
                        vm.ctx.new_str(key).into(),
                        vm.ctx.new_bool(mods[0]).into(), // alt
                        vm.ctx.new_bool(mods[1]).into(), // ctrl
                        vm.ctx.new_bool(mods[2]).into(), // shift
                        vm.ctx.new_bool(mods[3]).into(), // meta
                        vm.ctx.new_bool(mods[4]).into(), // repeat
                    ];
                    vm.ctx.new_tuple(elements).into()
                }
            }
        });

    let text_metrics_fn = vm.new_function(
        "text_metrics",
        |text: String, font_css: String, vm: &vm::VirtualMachine| -> PyObjectRef {
            let (width, height, x_offset, y_offset) = wasm_ffi::measure_text(&text, &font_css);
            let elements = vec![
                vm.ctx.new_float(width).into(),
                vm.ctx.new_float(height).into(),
                vm.ctx.new_float(x_offset).into(),
                vm.ctx.new_float(y_offset).into(),
            ];
            vm.ctx.new_tuple(elements).into()
        },
    );

    let load_bitmap_fn = vm.new_function(
        "load_bitmap",
        |_path: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            Err(vm.new_runtime_error("load_bitmap is not available in native mode".to_owned()))
        },
    );
    #[cfg(test)]
    let test_mark_atexit_ran_fn = vm.new_function(
        "__test_mark_atexit_ran",
        |vm: &vm::VirtualMachine| -> vm::PyObjectRef {
            REPL_ATEXIT_RAN.store(true, Ordering::SeqCst);
            vm.ctx.none()
        },
    );

    // A fresh dict takes any name, so setting one of these cannot fail.
    let dict = vm.ctx.new_dict();
    let set = |name: &str, value: vm::PyObjectRef| {
        dict.set_item(name, value, vm)
            .expect("a fresh dict takes a name");
    };
    set("show_svg", show_svg_fn.into());
    set("get_key_event", get_key_event_fn.into());
    set("text_metrics", text_metrics_fn.into());
    set("load_bitmap", load_bitmap_fn.into());
    #[cfg(test)]
    set("__test_mark_atexit_ran", test_mark_atexit_ran_fn.into());
    let module = vm.new_module("_spython_ffi", dict, None);

    let sys_modules = vm
        .sys_module
        .get_attr("modules", vm)
        .expect("sys.modules should exist");
    sys_modules
        .set_item("_spython_ffi", module.into(), vm)
        .expect("should be able to add to sys.modules");
}

/// Replace sys.stdout and sys.stderr with writers that route output
/// to the thread-local capture buffers (see `output.rs`).
#[cfg(feature = "capture")]
fn install_capture_writers(vm: &vm::VirtualMachine) {
    fn make_writer(vm: &vm::VirtualMachine, name: &str, write_fn: fn(&str)) -> vm::PyObjectRef {
        let write = vm.new_function("write", move |s: String| -> usize {
            let len = s.len();
            write_fn(&s);
            len
        });
        let flush = vm.new_function("flush", || {});
        let dict = vm.ctx.new_dict();
        dict.set_item("write", write.into(), vm).unwrap();
        dict.set_item("flush", flush.into(), vm).unwrap();
        vm.new_module(name, dict, None).into()
    }

    let stdout = make_writer(vm, "_capture_stdout", output::write_stdout);
    let stderr = make_writer(vm, "_capture_stderr", output::write_stderr);
    vm.sys_module
        .set_attr("stdout", stdout, vm)
        .expect("set sys.stdout");
    vm.sys_module
        .set_attr("stderr", stderr, vm)
        .expect("set sys.stderr");
}
