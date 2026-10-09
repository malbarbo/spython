//! What a run answers where there is no page: a drawing goes to stdout, and
//! no key is ever waiting.

use rustpython_vm::{PyResult, VirtualMachine};

/// Write the drawing to `sys.stdout`, as `print` would, so it comes out in
/// order with what the program printed and goes where the program sent it.
pub fn draw_svg(svg: &str, vm: &VirtualMachine) -> PyResult<()> {
    let stdout = vm.sys_module.get_attr("stdout", vm)?;
    // `print` writes nothing when there is no `sys.stdout`.
    if !vm.is_none(&stdout) {
        vm.call_method(&stdout, "write", (format!("{svg}\n"),))?;
    }
    Ok(())
}

/// Read the file at `path`.
pub fn load_bitmap(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Nothing is ever waiting, as nothing here reads keys.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    None
}
