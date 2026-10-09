//! What the page answers, through the `env` imports it hands the module.

/// The `env` imports, under the names the page exports them by.
mod imports {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn draw_svg(ptr: *const u8, len: usize);
        pub fn get_key_event(key_ptr: *mut u8, key_len: usize, mods_ptr: *mut u8) -> i32;
    }
}

/// Hand the drawing to the page, which puts it where whoever is watching
/// will see it.
pub fn draw_svg(svg: &str) {
    unsafe {
        imports::draw_svg(svg.as_ptr(), svg.len());
    }
}

/// The page reads no file.
pub fn load_bitmap(_path: &str) -> Result<Vec<u8>, String> {
    Err("load_bitmap is not available in the browser".to_owned())
}

/// Take the event the page is holding, if it is holding one.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    let mut key_buf = [0u8; 48];
    let mut mods = [0u8; 5];
    let event_type =
        unsafe { imports::get_key_event(key_buf.as_mut_ptr(), key_buf.len(), mods.as_mut_ptr()) };
    if event_type >= 3 {
        return None;
    }
    // The page writes the name and a nul after it, and a name as long as
    // the buffer gets no nul, so the cut is at most the whole buffer.
    let key_len = key_buf
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(key_buf.len());
    let name = key_buf
        .get(..key_len)
        .expect("a cut at the nul, or at the end of the buffer");
    let key = String::from_utf8_lossy(name).into_owned();
    let mods_bool = [
        mods[0] != 0,
        mods[1] != 0,
        mods[2] != 0,
        mods[3] != 0,
        mods[4] != 0,
    ];
    Some((event_type, key, mods_bool))
}
