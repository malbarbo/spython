from collections.abc import Sequence as _Sequence
from enum import Enum
from typing import TYPE_CHECKING as _TYPE_CHECKING

import _spython_image as _native  # type: ignore[import-not-found]

from spython.color import Color as _Color


class LineCap(Enum):
    BUTT = "butt"
    ROUND = "round"
    SQUARE = "square"


class LineJoin(Enum):
    MITER = "miter"
    ROUND = "round"
    BEVEL = "bevel"


class FillRule(Enum):
    NONZERO = "nonzero"
    EVENODD = "evenodd"


# The class lives in `_spython_image`, which ty cannot read, so ty reads this
# declaration instead.
if _TYPE_CHECKING:

    class Style:
        pass

else:
    Style = _native.Style


# Public API

none: Style = _native.join([])


def join(*styles: Style) -> Style:
    return _native.join(list(styles))


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
        None if dash_array is None else list(dash_array),
        linecap,
        linejoin,
    )
