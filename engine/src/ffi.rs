//! The `_spython_ffi` module: what Python reaches the host through.

use rustpython::vm;

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::host;

/// Set by the `__test_mark_atexit_ran` function that `register_ffi_module`
/// adds under test, so [`crate::repl`] can see that dropping a session ran
/// Python's atexit handlers.
#[cfg(test)]
pub(crate) static REPL_ATEXIT_RAN: AtomicBool = AtomicBool::new(false);

/// Register the `_spython_ffi` module with native `show_svg` and `get_key_event` functions.
pub(crate) fn register_ffi_module(vm: &vm::VirtualMachine) {
    use rustpython::vm::PyObjectRef;

    let show_svg_fn = vm.new_function(
        "show_svg",
        |svg: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            host::draw_svg(&svg);
            Ok(vm.ctx.none())
        },
    );

    let get_key_event_fn =
        vm.new_function("get_key_event", |vm: &vm::VirtualMachine| -> PyObjectRef {
            match host::get_key_event() {
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
            let (width, height, x_offset, y_offset) = host::text_metrics(&text, &font_css);
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
        |path: String, vm: &vm::VirtualMachine| -> vm::PyResult {
            let (width, height, data_uri) =
                host::load_bitmap(&path).map_err(|e| vm.new_runtime_error(e))?;
            let elements = vec![
                vm.ctx.new_float(width).into(),
                vm.ctx.new_float(height).into(),
                vm.ctx.new_str(data_uri).into(),
            ];
            Ok(vm.ctx.new_tuple(elements).into())
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
