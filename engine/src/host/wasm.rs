//! What the page answers, through the `env` imports it hands the module.

use rustpython_vm::{PyResult, VirtualMachine};

/// The `env` imports, under the names that the page exports.
mod imports {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn draw_svg(ptr: *const u8, len: usize);
        pub fn check_interrupt() -> i32;
    }
}

/// Hand the drawing to the page, which puts it where whoever is watching
/// will see it.
pub fn draw_svg(svg: &str, _vm: &VirtualMachine) -> PyResult<()> {
    unsafe {
        imports::draw_svg(svg.as_ptr(), svg.len());
    }
    Ok(())
}

pub fn stopped() -> bool {
    unsafe { imports::check_interrupt() != 0 }
}

pub fn interruptible<R>(f: impl FnOnce() -> R) -> R {
    /// Puts the switch back as it was, even when `f` panics.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            rustpython_vm::vm::set_interruptible(self.0);
        }
    }
    let _restore = Restore(rustpython_vm::vm::set_interruptible(true));
    f()
}
