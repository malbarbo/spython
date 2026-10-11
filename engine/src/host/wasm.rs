//! What the page answers, through the `env` imports it hands the module.

use rustpython_vm::{PyResult, VirtualMachine};

/// The `env` imports, under the names the page exports them by.
mod imports {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn draw_svg(ptr: *const u8, len: usize);
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

/// The page reads no file.
pub fn load_bitmap(_path: &str) -> Result<Vec<u8>, String> {
    Err("load_bitmap is not available in the browser".to_owned())
}
