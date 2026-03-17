//! WASM FFI for spython — env imports for SVG display and keyboard input.
//!
//! These functions are declared as WASM env imports and called from Python
//! via the `_spython_ffi` built-in module.

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn draw_svg(ptr: *const u8, len: usize);
    fn get_key_event(key_ptr: *mut u8, key_len: usize, mods_ptr: *mut u8) -> i32;
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
