from enum import Enum


class FontStyle(Enum):
    NORMAL = "normal"
    ITALIC = "italic"
    SLANT = "oblique"


class FontWeight(Enum):
    LIGHT = "lighter"
    REGULAR = "normal"
    BOLD = "bold"


# Module-level shorthands so users can write `style=ITALIC` instead of
# `style=FontStyle.ITALIC`.
NORMAL: FontStyle = FontStyle.NORMAL
ITALIC: FontStyle = FontStyle.ITALIC
SLANT: FontStyle = FontStyle.SLANT
LIGHT: FontWeight = FontWeight.LIGHT
REGULAR: FontWeight = FontWeight.REGULAR
BOLD: FontWeight = FontWeight.BOLD


class Font:
    def __init__(
        self,
        *,
        family: str = "sans-serif",
        size: float = 14.0,
        style: FontStyle = FontStyle.NORMAL,
        weight: FontWeight = FontWeight.REGULAR,
        underline: bool = False,
    ) -> None:
        self.family: str = family
        self.size: float = size
        self.style: FontStyle = style
        self.weight: FontWeight = weight
        self.underline: bool = underline


# The whole css shorthand, size and all, which the host hands to the canvas as
# it comes. Css fixes this order, and a size in front of the weight turns the
# rest into a family name.
def _to_css(font: Font) -> str:
    s: str = ""
    if font.style == FontStyle.ITALIC:
        s = "italic "
    elif font.style == FontStyle.SLANT:
        s = "oblique "
    w: str = ""
    if font.weight == FontWeight.BOLD:
        w = "bold "
    elif font.weight == FontWeight.LIGHT:
        w = "lighter "
    return s + w + _f(font.size) + "px " + font.family


def _f(v: float) -> str:
    r: str = f"{v:.6f}".rstrip("0").rstrip(".")
    if r == "-0":
        return "0"
    return r
