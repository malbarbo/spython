//! Native host: dispatches to the [`simage`] crate via a process-wide
//! [`simage::frontend::Frontend`]. The browser frontend has its own
//! counterpart in [`super::wasm`].
//!
//! Two display backends are available, chosen by the `SPYTHON_DISPLAY`
//! environment variable:
//! - `terminal` (default) — Kitty / Sixel / half-blocks via [`simage::terminal`].
//! - `window` — native OS window via [`simage::window`].
//!
//! The choice is read once on first use and cached for the process lifetime.
//! Capability probes ([`kitty_supported`], [`sixel_supported`],
//! [`text_blocks_supported`]) consult only the cached choice and the medium,
//! so they do not contend on the [`Frontend`] mutex.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use simage::event::InputEvent;
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

/// Process-wide frontend. Constructed lazily on first use.
fn frontend() -> &'static Mutex<Frontend> {
    static FRONTEND: OnceLock<Mutex<Frontend>> = OnceLock::new();
    FRONTEND.get_or_init(|| {
        Mutex::new(match display() {
            Display::Terminal => Frontend::terminal(),
            Display::Window => Frontend::window("spython"),
        })
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
    frontend().lock().unwrap().enter();
}

/// Notify the host that an animation loop has ended.
pub fn exit_animation() {
    frontend().lock().unwrap().exit();
}

/// Render a [`simage::ir::DrawList`] to the active backend.
pub fn show_image_dl(dl: &simage::ir::DrawList) {
    frontend().lock().unwrap().present(dl);
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

/// Block until the next input event from the active frontend. `deadline_ms`
/// caps the wait (relative to now); `None` means "wait forever".
///
/// Returns `(kind, key, [alt, ctrl, shift, meta, repeat])` or `None` if the
/// frontend has shut down (window closed, stdin EOF).
///
/// Event kinds: `0` = KeyPress, `1` = KeyDown, `2` = KeyUp, `3` = Vsync,
/// `4` = Close. For Vsync and Close, `key` is empty and modifiers are all
/// `false`.
pub fn wait_event(deadline_ms: Option<i64>) -> Option<(i32, String, [bool; 5])> {
    let deadline =
        deadline_ms.map(|ms| Instant::now() + Duration::from_millis(ms.max(0) as u64));
    let ev = frontend().lock().unwrap().wait_event(deadline)?;
    Some(input_event_to_tuple(ev))
}

fn input_event_to_tuple(ev: InputEvent) -> (i32, String, [bool; 5]) {
    match ev {
        InputEvent::Key(k) => {
            let kind = k.kind as i32;
            let mods = [k.alt(), k.ctrl(), k.shift(), k.meta(), k.repeat()];
            (kind, k.key, mods)
        }
        InputEvent::Vsync => (3, String::new(), [false; 5]),
        InputEvent::Close => (4, String::new(), [false; 5]),
    }
}
