//! What a program asks of the world around it. The page answers in `wasm`
//! through its `env` imports, and the operating system in `native`.
//!
//! Each function below carries the name of the import it wraps, and Python
//! reaches them through the `_spython_ffi` built-in module. Both targets
//! answer every one of them, under the name and with the meaning written
//! here.

/// Put a drawing where whoever is watching will see it: on the page under
/// WASM, and on stdout natively.
pub use target::draw_svg;

/// The kind of the event waiting on the page, the name of the key and the
/// modifiers that are on, or nothing when no event waits. Only the browser
/// reads keys, so a native run never has one.
pub use target::get_key_event;

/// The width and the height of the box around the text, and the offsets from
/// the middle of that box to the origin of an svg `<text>` element, which sits
/// at the start of the baseline. The browser measures, and natively sinteract
/// does, with the fonts that it draws.
pub use target::text_metrics;

/// The width and the height of the image at the path, and a data URI that
/// holds it, or the message of the error. Natively the image is a PNG, and
/// the page reads no file.
pub use target::load_bitmap;

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;
