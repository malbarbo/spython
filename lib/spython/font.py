from enum import Enum
from typing import TYPE_CHECKING as _TYPE_CHECKING

import _spython_image as _native  # type: ignore[import-not-found]


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


# The class lives in `_spython_image`, which ty cannot read, so ty reads this
# declaration instead.
if _TYPE_CHECKING:

    class Font:
        def __init__(
            self,
            *,
            family: str = "sans-serif",
            size: float = 14.0,
            style: FontStyle = FontStyle.NORMAL,
            weight: FontWeight = FontWeight.REGULAR,
            underline: bool = False,
        ) -> None: ...

        @property
        def family(self) -> str: ...

        @property
        def size(self) -> float: ...

        @property
        def style(self) -> FontStyle: ...

        @property
        def weight(self) -> FontWeight: ...

        @property
        def underline(self) -> bool: ...

else:
    Font = _native.Font
