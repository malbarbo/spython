//! Native host helpers: process-level concerns (panic hook, font metrics)
//! and the [`make_frontend`] builder consumed by the `_frontend.Frontend`
//! pyclass. The browser counterpart lives in [`super::wasm`].
//!
//! The display backend is chosen by the `SPYTHON_DISPLAY` environment
//! variable, read once on first use and cached:
//! - `terminal` (default) — Kitty / Sixel / half-blocks via [`simage::terminal`].
//! - `window` — native OS window via [`simage::window`].
//!
//! The Frontend itself is owned by Python (as a `_frontend.Frontend`
//! instance). There is no process-wide Frontend singleton — every call to
//! [`make_frontend`] returns a fresh struct, cheap to construct in either
//! mode (window mode allocates resources only on `enter()`).

use std::sync::OnceLock;

use simage::frontend::Frontend;

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

/// Construct a fresh [`Frontend`] for the selected display mode. The
/// caller owns the returned value.
pub fn make_frontend() -> Frontend {
    match display() {
        Display::Terminal => Frontend::terminal(),
        Display::Window => Frontend::window("spython"),
    }
}

/// Install a panic hook that restores any state owned by the active backend.
pub fn install_panic_hook() {
    match display() {
        Display::Terminal => simage::terminal::install_panic_hook(),
        Display::Window => simage::window::install_panic_hook(),
    }
}

/// Decode a CSS font string (as built by `spython.font._to_css`) into
/// `(family, weight, style)`. Inputs look like `"italic bold serif"` or
/// `"Liberation Mono"` — keywords come first, then the family name (which
/// may contain spaces). Unknown tokens are treated as part of the family,
/// numeric tokens override the weight (matches CSS).
fn parse_css_font(css: &str) -> (String, u16, simage::ir::FontStyle) {
    let mut style = simage::ir::FontStyle::Normal;
    let mut weight: u16 = 400;
    let mut family_parts: Vec<&str> = Vec::new();
    for token in css.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "italic" => style = simage::ir::FontStyle::Italic,
            "oblique" => style = simage::ir::FontStyle::Oblique,
            "normal" => {}
            "bold" | "bolder" => weight = 700,
            "lighter" => weight = 300,
            _ => {
                if let Ok(n) = token.parse::<u16>() {
                    weight = n;
                } else {
                    family_parts.push(token);
                }
            }
        }
    }
    (family_parts.join(" "), weight, style)
}

fn resolved_face(font_css: &str) -> simage::text::ResolvedFont {
    let (family, weight, style) = parse_css_font(font_css);
    simage::text::resolve(&family, weight, style)
}

/// Measure text width, honoring the requested family / weight / style
/// (CSS-formatted by `spython.font._to_css`). Falls back to Liberation Sans
/// when the family doesn't match an embedded face or a system one.
pub fn measure_text_width(text: &str, font: &str, size: i32) -> f64 {
    simage::text::measure_width_with(resolved_face(font).face(), text, size)
}

/// Measure text height — driven by font metrics, so the chosen face's
/// ascender/descender determine the box.
pub fn measure_text_height(text: &str, font: &str, size: i32) -> f64 {
    simage::text::measure_height_with(resolved_face(font).face(), text, size)
}

/// Measure text x-offset (`-width / 2`, so `text-anchor=start` renders
/// centered in the bounding box).
pub fn measure_text_x_offset(text: &str, font: &str, size: i32) -> f64 {
    simage::text::measure_x_offset_with(resolved_face(font).face(), text, size)
}

/// Measure text y-offset — the alphabetic baseline position relative to
/// the box center.
pub fn measure_text_y_offset(text: &str, font: &str, size: i32) -> f64 {
    simage::text::measure_y_offset_with(resolved_face(font).face(), text, size)
}
