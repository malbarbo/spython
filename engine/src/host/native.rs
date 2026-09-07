//! What a run answers where there is no page: a drawing goes to stdout, the
//! measuring is a guess, and no key is ever waiting.

/// Print the drawing, which is as far as a terminal takes one.
pub fn draw_svg(svg: &str) {
    println!("{svg}");
}

/// Guess the box: a character is 0.6 of the size wide, a line is the size
/// high, and the baseline sits 0.8 of the size below the top.
pub fn text_metrics(text: &str, font_css: &str) -> (f64, f64, f64, f64) {
    let size = font_size(font_css);
    let width = text.chars().count() as f64 * size * 0.6;
    let ascent = size * 0.8;
    (width, size, -width / 2.0, ascent - size / 2.0)
}

/// The size in pixels the css says. Without one the size is 14, what a `Font`
/// starts with.
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

/// Nothing is ever waiting, as nothing here reads keys.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    None
}
