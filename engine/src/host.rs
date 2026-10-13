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

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;
