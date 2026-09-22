//! What a run answers where there is no page: a drawing goes to stdout,
//! sinteract measures a text, and no key is ever waiting.

use sinteract::scene::FontStyle;

/// Print the drawing.
pub fn draw_svg(svg: &str) {
    println!("{svg}");
}

/// Measure the text in the face that the css picks, the one that sinteract
/// draws it with. A size that cannot draw measures nothing.
pub fn text_metrics(text: &str, font_css: &str) -> (f64, f64, f64, f64) {
    let font = CssFont::parse(font_css);
    match sinteract::text::measure(font.family, font.weight, font.style, font.size, text) {
        Some(m) => {
            let width = f64::from(m.width());
            (
                width,
                f64::from(m.height()),
                -width / 2.0,
                f64::from(m.baseline_y()),
            )
        }
        None => (0.0, 0.0, 0.0, 0.0),
    }
}

/// Nothing is ever waiting, as nothing here reads keys.
pub fn get_key_event() -> Option<(i32, String, [bool; 5])> {
    None
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
    fn text_metrics_puts_the_origin_at_the_start_of_the_baseline() {
        let (width, height, x, y) = text_metrics("spython", "14px sans-serif");
        assert!(width > 0.0 && height > 0.0);
        assert_eq!(x, -width / 2.0);
        assert!(y > 0.0 && y < height / 2.0);
    }
}
