class FontStyle:
    NORMAL: int = 0
    ITALIC: int = 1
    SLANT: int = 2


class FontWeight:
    LIGHT: int = 0
    REGULAR: int = 1
    BOLD: int = 2


class Font:
    def __init__(
        self,
        *,
        family: str = "sans-serif",
        size: float = 14.0,
        style: int = FontStyle.NORMAL,
        weight: int = FontWeight.REGULAR,
        underline: bool = False,
    ) -> None:
        self.family: str = family
        self.size: float = size
        self.style: int = style
        self.weight: int = weight
        self.underline: bool = underline


def _style_to_svg(s: int) -> str:
    if s == FontStyle.ITALIC:
        return "italic"
    if s == FontStyle.SLANT:
        return "oblique"
    return "normal"


def _weight_to_svg(w: int) -> str:
    if w == FontWeight.LIGHT:
        return "lighter"
    if w == FontWeight.BOLD:
        return "bold"
    return "normal"


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
    return s + w + str(font.size) + "px " + font.family
