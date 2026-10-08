//! What a run answers where there is no page: a drawing goes to stdout, and
//! no key is ever waiting.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use sinteract::asset::image_head;

/// Print the drawing.
pub fn draw_svg(svg: &str) {
    println!("{svg}");
}

/// Read the image at `path` into a data URI.
pub fn load_bitmap(path: &str) -> Result<String, String> {
    let blob = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let head = image_head(&blob).map_err(|e| format!("{path}: {e}"))?;
    Ok(format!("data:{};base64,{}", head.mime, B64.encode(&blob)))
}

/// Nothing is ever waiting, as nothing here reads keys.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    None
}
