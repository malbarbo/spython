from collections.abc import Sequence as _Sequence
from enum import Enum
from typing import TYPE_CHECKING as _TYPE_CHECKING
from typing import Never as _Never

import _spython_image as _native  # type: ignore[import-not-found]

from spython.color import Color as _Color


class LineCap(Enum):
    BUTT = _native.LINECAP_BUTT
    ROUND = _native.LINECAP_ROUND
    SQUARE = _native.LINECAP_SQUARE


class LineJoin(Enum):
    MITER = _native.LINEJOIN_MITER
    ROUND = _native.LINEJOIN_ROUND
    BEVEL = _native.LINEJOIN_BEVEL


class FillRule(Enum):
    NONZERO = _native.FILLRULE_NONZERO
    EVENODD = _native.FILLRULE_EVENODD


# The class lives in `_spython_image`, which ty cannot read, so ty reads this
# declaration instead.
if _TYPE_CHECKING:

    class Style:
        # No value fits `_Never`, so the checker rejects any call to Style.
        def __init__(self, _: _Never) -> None: ...

else:
    Style = _native.Style


# Public API

none: Style = _native.join([])


def join(*styles: Style) -> Style:
    return _native.join(styles)


def fill(
    c: _Color, *, opacity: float | None = None, rule: FillRule | None = None
) -> Style:
    return _native.fill(c, opacity, rule)


def stroke(
    c: _Color,
    *,
    width: float | None = None,
    opacity: float | None = None,
    dash_array: _Sequence[float] | None = None,
    linecap: LineCap | None = None,
    linejoin: LineJoin | None = None,
) -> Style:
    return _native.stroke(
        c,
        width,
        opacity,
        dash_array,
        linecap,
        linejoin,
    )
