from enum import Enum

from spython._check import _non_negative


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
        _non_negative(size=size)
        self.family: str = family
        self.size: float = size
        self.style: FontStyle = style
        self.weight: FontWeight = weight
        self.underline: bool = underline
