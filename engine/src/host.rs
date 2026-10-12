//! What a program asks of the world around it. The page answers in `wasm`
//! through its `env` imports, and the operating system in `native`.
//!
//! Each function that a target answers carries the name of the import it
//! wraps, and Python reaches them through the `_spython_ffi` built-in
//! module.
//! Both targets answer every one of them, under the name and with the meaning
//! written here.

/// Put a drawing where whoever is watching will see it: on the page under
/// WASM, and on `sys.stdout` natively.
pub use target::draw_svg;

/// Returns `true` if the user stopped the run, `false` otherwise. It asks
/// `check_interrupt` of the page. The Stop of the page stays up until the
/// next run, so the answer holds until the end of the call where the Stop
/// arrived. Natively a Ctrl-C is a signal, which Python raises as
/// `KeyboardInterrupt`, and this is always `false`.
pub use target::stopped;

/// Runs `f`, the code of the student, where a Stop interrupts it. The rest
/// of what spython runs has to finish even with the Stop still up. That
/// includes the start of an interpreter, which panics on any exception, the
/// displayhook, and the clean-up after a run. Natively a Ctrl-C interrupts
/// any code, and this just runs `f`.
pub use target::interruptible;

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;
