//! Native host: dispatches to the [`simage`] crate. The browser frontend
//! has its own counterpart in [`super::wasm`].

/// Show an SVG image — prints to stdout (the canonical text fallback used
/// when no graphics protocol is available).
pub fn show_svg(svg: &str) {
    simage::show_svg(svg);
}

/// Whether the host terminal speaks the Kitty graphics protocol.
pub fn kitty_supported() -> bool {
    simage::kitty_supported()
}

/// Whether the host terminal supports DEC Sixel.
pub fn sixel_supported() -> bool {
    simage::sixel_supported()
}

/// Whether the host terminal supports the truecolor half-blocks fallback.
pub fn text_blocks_supported() -> bool {
    simage::text_blocks_supported()
}

/// Notify the host that an animation loop is about to start (e.g. `World.run`).
/// simage enters alt-screen + raw mode.
pub fn enter_animation() {
    simage::enter_animation();
}

/// Notify the host that an animation loop has ended.
pub fn exit_animation() {
    simage::exit_animation();
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
    simage::poll_key_event()
}
