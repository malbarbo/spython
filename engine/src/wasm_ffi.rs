//! WASM FFI for spython — env imports for SVG display and keyboard input.
//!
//! These functions are declared as WASM env imports and called from Python
//! via the `_spython_ffi` built-in module.

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn draw_svg(ptr: *const u8, len: usize);
    fn get_key_event(key_ptr: *mut u8, key_len: usize, mods_ptr: *mut u8) -> i32;
    fn text_width(
        text: *const u8,
        text_len: usize,
        font_css: *const u8,
        font_css_len: usize,
    ) -> f64;
    fn text_height(
        text: *const u8,
        text_len: usize,
        font_css: *const u8,
        font_css_len: usize,
    ) -> f64;
    fn text_x_offset(
        text: *const u8,
        text_len: usize,
        font_css: *const u8,
        font_css_len: usize,
    ) -> f64;
    fn text_y_offset(
        text: *const u8,
        text_len: usize,
        font_css: *const u8,
        font_css_len: usize,
    ) -> f64;
}

/// Show an SVG image. On WASM, calls the env import; on native, prints to stdout.
pub fn show_svg(svg: &str) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        draw_svg(svg.as_ptr(), svg.len());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("{svg}");
    }
}

/// Parse font size from CSS font string like "14.0px sans-serif" or "italic bold 20.0px serif".
#[cfg(not(target_arch = "wasm32"))]
fn parse_font_size(font_css: &str) -> f64 {
    for part in font_css.split_whitespace() {
        if let Some(num_str) = part.strip_suffix("px")
            && let Ok(size) = num_str.parse::<f64>()
        {
            return size;
        }
    }
    14.0
}

/// Measure text width. On WASM, uses OffscreenCanvas; on native, uses 0.6 ratio.
pub fn measure_text_width(text: &str, font_css: &str) -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { text_width(text.as_ptr(), text.len(), font_css.as_ptr(), font_css.len()) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let size = parse_font_size(font_css);
        text.len() as f64 * size * 0.6
    }
}

/// Measure text height. On WASM, uses OffscreenCanvas; on native, uses font size.
pub fn measure_text_height(text: &str, font_css: &str) -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { text_height(text.as_ptr(), text.len(), font_css.as_ptr(), font_css.len()) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = text;
        parse_font_size(font_css)
    }
}

/// Measure text x offset. On WASM, uses OffscreenCanvas; on native, returns 0.0.
pub fn measure_text_x_offset(text: &str, font_css: &str) -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { text_x_offset(text.as_ptr(), text.len(), font_css.as_ptr(), font_css.len()) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (text, font_css);
        0.0
    }
}

/// Measure text y offset. On WASM, uses OffscreenCanvas; on native, returns 0.0.
pub fn measure_text_y_offset(text: &str, font_css: &str) -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { text_y_offset(text.as_ptr(), text.len(), font_css.as_ptr(), font_css.len()) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (text, font_css);
        0.0
    }
}

/// Poll for a keyboard event. Returns None if no event is pending.
/// On native, always returns None.
pub fn poll_key_event() -> Option<(i32, String, [bool; 5])> {
    #[cfg(target_arch = "wasm32")]
    {
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
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}
