//! What a run answers where there is no page. A drawing goes to stdout.

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
