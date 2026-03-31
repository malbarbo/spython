from spython import color as _color
from spython.color import Color as _Color


class _Fill:
    def __init__(self, c: _Color) -> None:
        self.color: _Color = c


class _FillOpacity:
    def __init__(self, value: float) -> None:
        self.value: float = max(0.0, min(1.0, value))


class _FillRule:
    def __init__(self, value: str) -> None:
        self.value: str = value


class _Stroke:
    def __init__(self, c: _Color) -> None:
        self.color: _Color = c


class _StrokeWidth:
    def __init__(self, value: float) -> None:
        self.value: float = max(0.0, value)


class _StrokeOpacity:
    def __init__(self, value: float) -> None:
        self.value: float = max(0.0, min(1.0, value))


class _StrokeDashArray:
    def __init__(self, values: list[int]) -> None:
        self.values: list[int] = [max(0, v) for v in values]


class _StrokeLineCap:
    def __init__(self, value: str) -> None:
        self.value: str = value


class _StrokeLineJoin:
    def __init__(self, value: str) -> None:
        self.value: str = value


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

    def to_svg(self) -> str:
        attrs: list[_Attr] = self.attrs
        if _has_outline(self) and not _has_fill(self):
            attrs = [_Fill(_color.NONE)] + attrs
        svg: str = ""
        for a in attrs:
            svg = svg + _attribute_to_svg(a)
        if _has_outline(self) and not _has_stroke_line_cap(self):
            svg = svg + _attribs("stroke-linecap", "round")
        if _has_outline(self) and not _has_stroke_line_join(self):
            svg = (
                svg
                + _attribs("stroke-linejoin", "miter")
                + _attrib("stroke-miterlimit", 10.0)
            )
        return svg


def _has_fill(style: Style) -> bool:
    for a in style.attrs:
        if isinstance(a, _Fill):
            return True
    return False


def _has_outline(style: Style) -> bool:
    for a in style.attrs:
        if isinstance(a, _Stroke):
            return True
    return False


def _has_stroke_line_cap(style: Style) -> bool:
    for a in style.attrs:
        if isinstance(a, _StrokeLineCap):
            return True
    return False


def _has_stroke_line_join(style: Style) -> bool:
    for a in style.attrs:
        if isinstance(a, _StrokeLineJoin):
            return True
    return False


def _has_stroke_width(style: Style) -> bool:
    for a in style.attrs:
        if isinstance(a, _StrokeWidth):
            return True
    return False


def outline_offset(style: Style) -> float:
    if _has_outline(style) and not _has_fill(style) and not _has_stroke_width(style):
        return 0.5
    return 0.0


def _attribute_to_svg(a: _Attr) -> str:
    if isinstance(a, _Fill):
        return _attribs("fill", a.color.to_svg())
    if isinstance(a, _FillOpacity):
        return _attrib("fill-opacity", a.value)
    if isinstance(a, _FillRule):
        return _attribs("fill-rule", a.value)
    if isinstance(a, _Stroke):
        return _attribs("stroke", a.color.to_svg())
    if isinstance(a, _StrokeLineCap):
        return _attribs("stroke-linecap", a.value)
    if isinstance(a, _StrokeLineJoin):
        return _attribs("stroke-linejoin", a.value)
    if isinstance(a, _StrokeDashArray):
        return _attribs("stroke-dasharray", ", ".join(str(v) for v in a.values))
    if isinstance(a, _StrokeOpacity):
        return _attrib("stroke-opacity", a.value)
    if isinstance(a, _StrokeWidth):
        return _attrib("stroke-width", a.value)
    return ""


def _attrib(name: str, value: float) -> str:
    return name + '="' + str(value) + '" '


def _attribs(name: str, value: str) -> str:
    return name + '="' + value + '" '


# Public API

none: Style = Style([])


def join(*styles: Style) -> Style:
    attrs: list[_Attr] = []
    for s in styles:
        attrs = attrs + s.attrs
    return Style(attrs)


def fill(c: _Color, *, opacity: float | None = None, rule: str | None = None) -> Style:
    attrs: list[_Attr] = [_Fill(c)]
    if opacity is not None:
        attrs.append(_FillOpacity(max(0.0, min(1.0, float(opacity)))))
    if rule is not None:
        attrs.append(_FillRule(rule))
    return Style(attrs)


def stroke(
    c: _Color,
    *,
    width: float | None = None,
    opacity: float | None = None,
    dash_array: list[int] | None = None,
    linecap: str | None = None,
    linejoin: str | None = None,
) -> Style:
    attrs: list[_Attr] = [_Stroke(c)]
    if width is not None:
        attrs.append(_StrokeWidth(max(0.0, float(width))))
    if opacity is not None:
        attrs.append(_StrokeOpacity(max(0.0, min(1.0, float(opacity)))))
    if dash_array is not None:
        attrs.append(_StrokeDashArray([max(0, v) for v in dash_array]))
    if linecap is not None:
        attrs.append(_StrokeLineCap(linecap))
    if linejoin is not None:
        attrs.append(_StrokeLineJoin(linejoin))
    return Style(attrs)
