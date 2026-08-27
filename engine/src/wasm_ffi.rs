//! WASM FFI for spython — env imports for SVG display and keyboard input.
//!
//! These functions are declared as WASM env imports and called from Python
//! via the `_spython_ffi` built-in module.

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn draw_svg(ptr: *const u8, len: usize);
    fn get_key_event(key_ptr: *mut u8, key_len: usize, mods_ptr: *mut u8) -> i32;
    /// Writes the width, the height, the horizontal offset and the vertical
    /// offset into `out`, which holds four f64. The font is the whole css
    /// shorthand, size and all, and the page hands it to the canvas as it
    /// comes.
    fn text_metrics(
        text: *const u8,
        text_len: usize,
        font: *const u8,
        font_len: usize,
        out: *mut f64,
    );
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

/// The width and the height of the box around the text, and the offsets from
/// the middle of that box to the origin of an svg `<text>` element, which sits
/// at the start of the baseline. The browser measures, and natively a guess
/// stands in.
pub fn measure_text(text: &str, font_css: &str) -> (f64, f64, f64, f64) {
    #[cfg(target_arch = "wasm32")]
    {
        let mut out = [0.0f64; 4];
        unsafe {
            text_metrics(
                text.as_ptr(),
                text.len(),
                font_css.as_ptr(),
                font_css.len(),
                out.as_mut_ptr(),
            );
        }
        (out[0], out[1], out[2], out[3])
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // A character is 0.6 of the size wide, a line is the size high, and
        // the baseline sits 0.8 of the size below the top.
        let size = font_size(font_css);
        let width = text.chars().count() as f64 * size * 0.6;
        let ascent = size * 0.8;
        (width, size, -width / 2.0, ascent - size / 2.0)
    }
}

/// The size in pixels the css says. Without one the size is 14, what a `Font`
/// starts with.
#[cfg(not(target_arch = "wasm32"))]
fn font_size(font_css: &str) -> f64 {
    for part in font_css.split_whitespace() {
        if let Some(digits) = part.strip_suffix("px")
            && let Ok(size) = digits.parse::<f64>()
        {
            return size;
        }
    }
    14.0
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
