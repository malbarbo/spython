//! What a run answers where there is no page: a drawing goes to stdout, and
//! no key is ever waiting.

/// Print the drawing.
pub fn draw_svg(svg: &str) {
    println!("{svg}");
}

/// Read the file at `path`.
pub fn load_bitmap(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Nothing is ever waiting, as nothing here reads keys.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    None
}
