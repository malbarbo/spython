from collections.abc import Sequence as _Sequence
from enum import Enum

from spython._check import _between, _non_negative
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


class _Fill:
    def __init__(self, c: _Color) -> None:
        self.color: _Color = c


class _FillOpacity:
    def __init__(self, value: float) -> None:
        self.value: float = value


class _FillRule:
    def __init__(self, value: FillRule) -> None:
        self.value: FillRule = value


class _Stroke:
    def __init__(self, c: _Color) -> None:
        self.color: _Color = c


class _StrokeWidth:
    def __init__(self, value: float) -> None:
        self.value: float = value


class _StrokeOpacity:
    def __init__(self, value: float) -> None:
        self.value: float = value


class _StrokeDashArray:
    def __init__(self, values: _Sequence[float]) -> None:
        self.values: list[float] = list(values)


class _StrokeLineCap:
    def __init__(self, value: LineCap) -> None:
        self.value: LineCap = value


class _StrokeLineJoin:
    def __init__(self, value: LineJoin) -> None:
        self.value: LineJoin = value


_Attr = (
    _Fill
    | _FillOpacity
    | _FillRule
    | _Stroke
    | _StrokeWidth
    | _StrokeOpacity
    | _StrokeDashArray
    | _StrokeLineCap
    | _StrokeLineJoin
)


class Style:
    def __init__(self, attrs: list[_Attr]) -> None:
        self.attrs: list[_Attr] = attrs


def _scale(style: Style, factor: float) -> Style:
    """`style` for a shape scaled by `factor`. A stroke with no width of its
    own stays one pixel wide, as an outline does in 2htdp."""
    attrs: list[_Attr] = []
    for a in style.attrs:
        if isinstance(a, _StrokeWidth):
            attrs.append(_StrokeWidth(a.value * factor))
        elif isinstance(a, _StrokeDashArray):
            attrs.append(_StrokeDashArray([v * factor for v in a.values]))
        else:
            attrs.append(a)
    return Style(attrs)


# Public API

none: Style = Style([])


def join(*styles: Style) -> Style:
    attrs: list[_Attr] = []
    for s in styles:
        attrs = attrs + s.attrs
    return Style(attrs)


def fill(
    c: _Color, *, opacity: float | None = None, rule: FillRule | None = None
) -> Style:
    attrs: list[_Attr] = [_Fill(c)]
    if opacity is not None:
        _between(0.0, 1.0, opacity=opacity)
        attrs.append(_FillOpacity(opacity))
    if rule is not None:
        attrs.append(_FillRule(rule))
    return Style(attrs)


def stroke(
    c: _Color,
    *,
    width: float | None = None,
    opacity: float | None = None,
    dash_array: _Sequence[float] | None = None,
    linecap: LineCap | None = None,
    linejoin: LineJoin | None = None,
) -> Style:
    attrs: list[_Attr] = [_Stroke(c)]
    if width is not None:
        _non_negative(width=width)
        attrs.append(_StrokeWidth(width))
    if opacity is not None:
        _between(0.0, 1.0, opacity=opacity)
        attrs.append(_StrokeOpacity(opacity))
    if dash_array is not None:
        for value in dash_array:
            _non_negative(dash_array=value)
        attrs.append(_StrokeDashArray(dash_array))
    if linecap is not None:
        attrs.append(_StrokeLineCap(linecap))
    if linejoin is not None:
        attrs.append(_StrokeLineJoin(linejoin))
    return Style(attrs)
