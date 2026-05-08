//! Native host: dispatches to the [`simage`] crate. The browser frontend
//! has its own counterpart in [`super::wasm`].
//!
//! Two display backends are available, chosen by the `SPYTHON_DISPLAY`
//! environment variable:
//! - `terminal` (default) — Kitty / Sixel / half-blocks via [`simage::terminal`].
//! - `window` — native OS window via [`simage::window`].
//!
//! The choice is read once on first use and cached for the process lifetime.

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Display {
    Terminal,
    Window,
}

fn display() -> Display {
    static CHOICE: OnceLock<Display> = OnceLock::new();
    *CHOICE.get_or_init(|| match std::env::var("SPYTHON_DISPLAY").as_deref() {
        Ok("window") => Display::Window,
        _ => Display::Terminal,
    })
}

/// Show an SVG image — prints to stdout (the canonical text fallback used
/// when no graphics protocol is available).
pub fn show_svg(svg: &str) {
    match display() {
        Display::Terminal => simage::terminal::show_svg(svg),
        // Window backend renders rasterized DrawLists; SVG arrives only on
        // hosts without graphics support, so the SVG fallback path is moot
        // when a window is open. Print so REPL displayhook still works.
        Display::Window => println!("{svg}"),
    }
}

/// Whether the host terminal speaks the Kitty graphics protocol.
pub fn kitty_supported() -> bool {
    matches!(display(), Display::Terminal) && simage::terminal::kitty_supported()
}

/// Whether the host terminal supports DEC Sixel.
pub fn sixel_supported() -> bool {
    matches!(display(), Display::Terminal) && simage::sixel::sixel_supported()
}

/// Whether the host supports raster graphics output.
///
/// In window mode this is always `true` (the window is itself a raster
/// surface). In terminal mode it reflects whether truecolor half-blocks
/// are usable as the last-resort terminal renderer.
pub fn text_blocks_supported() -> bool {
    match display() {
        Display::Terminal => simage::terminal::text_blocks_supported(),
        Display::Window => true,
    }
}

/// Notify the host that an animation loop is about to start (e.g. `World.run`).
pub fn enter_animation() {
    match display() {
        Display::Terminal => simage::terminal::enter_animation(),
        Display::Window => simage::window::enter_animation("spython"),
    }
}

/// Notify the host that an animation loop has ended.
pub fn exit_animation() {
    match display() {
        Display::Terminal => simage::terminal::exit_animation(),
        Display::Window => simage::window::exit_animation(),
    }
}

/// Render a [`simage::ir::DrawList`] to the active backend.
pub fn show_image_dl(dl: &simage::ir::DrawList) {
    match display() {
        Display::Terminal => simage::terminal::show_image_dl(dl),
        Display::Window => simage::window::show_image_dl(dl),
    }
}

/// Install a panic hook that restores any state owned by the active backend.
pub fn install_panic_hook() {
    match display() {
        Display::Terminal => simage::terminal::install_panic_hook(),
        Display::Window => simage::window::install_panic_hook(),
    }
}

/// Measure text width in the embedded Liberation Sans font.
pub fn measure_text_width(text: &str, _font: &str, size: i32) -> f64 {
    simage::text::measure_width(text, size)
}

/// Measure text height in the embedded Liberation Sans font.
pub fn measure_text_height(text: &str, _font: &str, size: i32) -> f64 {
    simage::text::measure_height(text, size)
}

/// Measure text x-offset (`-width / 2`, so `text-anchor=start` renders
/// centered in the bounding box).
pub fn measure_text_x_offset(text: &str, _font: &str, size: i32) -> f64 {
    simage::text::measure_x_offset(text, size)
}

/// Measure text y-offset — the alphabetic baseline position relative to
/// the box center.
pub fn measure_text_y_offset(text: &str, _font: &str, size: i32) -> f64 {
    simage::text::measure_y_offset(text, size)
}

/// Poll for a keyboard event. Returns `None` if no event is pending.
pub fn poll_key_event() -> Option<(i32, String, [bool; 5])> {
    match display() {
        Display::Terminal => simage::terminal::poll_key_event(),
        Display::Window => simage::window::poll_key_event(),
    }
}
