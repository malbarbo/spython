//! Host-environment integration — the bridge between spython's Python FFI
//! (`_spython_ffi`) and whatever surrounds the engine at runtime.
//!
//! Two hosts are supported:
//! - **Native** — dispatches to the [`simage`] crate (terminal renderer,
//!   capability probe, font metrics, PDF). See [`native`].
//! - **WASM** — calls into JS via env imports; the browser frontend
//!   supplies its own canvas-based pipeline. See [`wasm`].
//!
//! Each submodule provides the *same* public function set so
//! `register_ffi_module` (in `lib.rs`) can wire them into Python uniformly.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
