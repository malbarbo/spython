//! What a program asks of the world around it. The page answers in `wasm`
//! through its `env` imports, and the operating system in `native`. A text
//! is measured here, the same way on both.
//!
//! Each function that a target answers carries the name of the import it
//! wraps, and Python reaches them through the `_spython_ffi` and
//! `_spython_scene` built-in modules.
//! Both targets answer every one of them, under the name and with the meaning
//! written here.

use sinteract::scene::FontStyle;

/// Put a drawing where whoever is watching will see it: on the page under
/// WASM, and on stdout natively.
pub use target::draw_svg;

/// The kind of the event waiting on the page, the name of the key and the
/// modifiers that are on, or nothing when no event waits. Only the browser
/// reads keys, so a native run never has one.
pub use target::get_key_event;

/// The bytes of the file at the path, or the message of the error. The page
/// reads no file.
pub use target::load_bitmap;

/// The width and the height of the box around the text. sinteract measures,
/// with the fonts that it embeds and uses to draw, so a text has the same
/// size on every target. A size that cannot draw measures nothing.
pub fn text_metrics(text: &str, font_css: &str) -> (f64, f64) {
    let font = CssFont::parse(font_css);
    match sinteract::text::measure(font.family, font.weight, font.style, font.size, text) {
        Some(m) => (f64::from(m.width()), f64::from(m.height())),
        None => (0.0, 0.0),
    }
}

/// The font of a css shorthand from `_to_css` of `spython.font`, which
/// writes the style, the weight, the size in pixels and the family, in that
/// order.
struct CssFont<'a> {
    style: FontStyle,
    weight: u16,
    size: f32,
    family: &'a str,
}

impl<'a> CssFont<'a> {
    /// Without a size the font is the 14 pixels that a `Font` starts with.
    fn parse(css: &'a str) -> Self {
        let mut font = CssFont {
            style: FontStyle::Normal,
            weight: 400,
            size: 14.0,
            family: "",
        };
        let mut rest = css.trim_start();
        while let Some((word, after)) = rest.split_once(' ') {
            match word {
                "italic" => font.style = FontStyle::Italic,
                "oblique" => font.style = FontStyle::Oblique,
                "bold" => font.weight = 700,
                "lighter" => font.weight = 300,
                _ => {
                    if let Some(size) = word.strip_suffix("px").and_then(|d| d.parse().ok()) {
                        font.size = size;
                        font.family = after;
                    }
                    break;
                }
            }
            rest = after.trim_start();
        }
        font
    }
}

#[cfg(target_arch = "wasm32")]
#[path = "host/wasm.rs"]
mod target;
#[cfg(not(target_arch = "wasm32"))]
#[path = "host/native.rs"]
mod target;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_font_reads_the_style_the_weight_the_size_and_the_family() {
        let font = CssFont::parse("italic bold 20px Liberation Serif");
        assert_eq!(font.style, FontStyle::Italic);
        assert_eq!(font.weight, 700);
        assert_eq!(font.size, 20.0);
        assert_eq!(font.family, "Liberation Serif");
    }

    #[test]
    fn css_font_without_a_style_and_a_weight_is_normal() {
        let font = CssFont::parse("14.5px sans-serif");
        assert_eq!(font.style, FontStyle::Normal);
        assert_eq!(font.weight, 400);
        assert_eq!(font.size, 14.5);
        assert_eq!(font.family, "sans-serif");
    }

    #[test]
    fn text_metrics_measures_a_text() {
        let (width, height) = text_metrics("spython", "14px sans-serif");
        assert!(width > 0.0 && height > 0.0);
    }

    #[test]
    fn text_metrics_of_a_size_that_cannot_draw_is_nothing() {
        assert_eq!(text_metrics("spython", "0px sans-serif"), (0.0, 0.0));
    }
}
