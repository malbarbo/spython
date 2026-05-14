from enum import Enum


class FontStyle(Enum):
    NORMAL = "normal"
    ITALIC = "italic"
    OBLIQUE = "oblique"


# Pesos seguem a convenção CSS (100 a 900 em incrementos de 100). As
# constantes nomeadas cobrem os três pesos mais usados; valores
# intermediários (ex.: 500) só rendem visualmente quando a família
# carregada (geralmente uma fonte do sistema) tiver a variante
# correspondente.
LIGHT: int = 300
REGULAR: int = 400
BOLD: int = 700


# Module-level shorthands so users can write `style=ITALIC` instead of
# `style=FontStyle.ITALIC`.
NORMAL: FontStyle = FontStyle.NORMAL
ITALIC: FontStyle = FontStyle.ITALIC
OBLIQUE: FontStyle = FontStyle.OBLIQUE


class Font:
    def __init__(
        self,
        *,
        family: str = "sans-serif",
        size: float = 14.0,
        style: FontStyle = FontStyle.NORMAL,
        weight: int = REGULAR,
        underline: bool = False,
    ) -> None:
        self.family: str = family
        self.size: float = size
        self.style: FontStyle = style
        self.weight: int = weight
        self.underline: bool = underline


def _to_css(font: Font) -> str:
    s: str = ""
    if font.style == FontStyle.ITALIC:
        s = "italic "
    elif font.style == FontStyle.OBLIQUE:
        s = "oblique "
    w: str = ""
    if font.weight >= 600:
        w = "bold "
    elif font.weight < 400:
        w = "lighter "
    return s + w + font.family
