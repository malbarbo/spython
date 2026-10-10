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

/// The kind of the event waiting on the page, the name of the key and the
/// modifiers that are on, or nothing when no event waits. Only the browser
/// reads keys, so a native run never has one.
pub use target::get_key_event;

/// The bytes of the file at the path, or the message of the error. The page
/// reads no file.
pub use target::load_bitmap;

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;
