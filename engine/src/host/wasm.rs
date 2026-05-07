//! WASM host: dispatches to JS via the env imports declared below. The
//! browser frontend provides the canvas-based renderer and OffscreenCanvas
//! text metrics. The native counterpart lives in [`super::native`].

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn draw_svg(ptr: *const u8, len: usize);
    fn get_key_event(key_ptr: *mut u8, key_len: usize, mods_ptr: *mut u8) -> i32;
    fn text_width(
        text: *const u8,
        text_len: usize,
        font: *const u8,
        font_len: usize,
        size: i32,
    ) -> f64;
    fn text_height(
        text: *const u8,
        text_len: usize,
        font: *const u8,
        font_len: usize,
        size: i32,
    ) -> f64;
    fn text_x_offset(
        text: *const u8,
        text_len: usize,
        font: *const u8,
        font_len: usize,
        size: i32,
    ) -> f64;
    fn text_y_offset(
        text: *const u8,
        text_len: usize,
        font: *const u8,
        font_len: usize,
        size: i32,
    ) -> f64;
}

/// Show an SVG image via the JS `draw_svg` env import.
pub fn show_svg(svg: &str) {
    unsafe { draw_svg(svg.as_ptr(), svg.len()) };
}

/// JS-canvas hosts do not advertise the Kitty graphics protocol.
pub fn kitty_supported() -> bool {
    false
}

/// JS-canvas hosts do not advertise DEC Sixel.
pub fn sixel_supported() -> bool {
    false
}

/// JS-canvas hosts do not advertise the half-blocks fallback.
pub fn text_blocks_supported() -> bool {
    false
}

/// No-op on WASM — animation lifecycle is owned by the JS frontend.
pub fn enter_animation() {}

/// No-op on WASM.
pub fn exit_animation() {}

/// Measure text width via OffscreenCanvas.
pub fn measure_text_width(text: &str, font: &str, size: i32) -> f64 {
    unsafe { text_width(text.as_ptr(), text.len(), font.as_ptr(), font.len(), size) }
}

/// Measure text height via OffscreenCanvas.
pub fn measure_text_height(text: &str, font: &str, size: i32) -> f64 {
    unsafe { text_height(text.as_ptr(), text.len(), font.as_ptr(), font.len(), size) }
}

/// Measure text x-offset via OffscreenCanvas.
pub fn measure_text_x_offset(text: &str, font: &str, size: i32) -> f64 {
    unsafe { text_x_offset(text.as_ptr(), text.len(), font.as_ptr(), font.len(), size) }
}

/// Measure text y-offset via OffscreenCanvas.
pub fn measure_text_y_offset(text: &str, font: &str, size: i32) -> f64 {
    unsafe { text_y_offset(text.as_ptr(), text.len(), font.as_ptr(), font.len(), size) }
}

/// Poll for a keyboard event via the JS `get_key_event` env import.
pub fn poll_key_event() -> Option<(i32, String, [bool; 5])> {
    let mut key_buf = [0u8; 48];
    let mut mods = [0u8; 5];
    let event_type =
        unsafe { get_key_event(key_buf.as_mut_ptr(), key_buf.len(), mods.as_mut_ptr()) };
    if event_type >= 3 {
        return None;
    }
    let key_len = key_buf
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(key_buf.len());
    let key = String::from_utf8_lossy(&key_buf[..key_len]).into_owned();
    let mods_bool = [
        mods[0] != 0,
        mods[1] != 0,
        mods[2] != 0,
        mods[3] != 0,
        mods[4] != 0,
    ];
    Some((event_type, key, mods_bool))
}
