//! What a program asks of the world around it. The page answers in `wasm`
//! through its `env` imports, and the operating system in `native`.
//!
//! Each function that a target answers carries the name of the import it
//! wraps, and Python reaches them through the `_spython_ffi` and
//! `_spython_scene` built-in modules.
//! Both targets answer every one of them, under the name and with the meaning
//! written here.

/// Put a drawing where whoever is watching will see it: on the page under
/// WASM, and on `sys.stdout` natively.
pub use target::draw_svg;

/// Whether the user stopped the run, wrapping `check_interrupt`. The page's
/// Stop stays up until its next run, so this answers for the whole of the
/// call that the Stop came in. Natively a Ctrl-C is a signal, which Python
/// raises as `KeyboardInterrupt`, and this is always `false`.
pub use target::stopped;

/// Runs `f`, the student's code, where a Stop interrupts it. Everything
/// else spython runs -- the start of an interpreter, which panics on any
/// exception, the displayhook, the clean-up after a run -- has to finish
/// even with the Stop still up, so a Stop reaches only the code in here.
/// Natively a Ctrl-C reaches any code, and this just runs `f`.
pub use target::interruptible;

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;
