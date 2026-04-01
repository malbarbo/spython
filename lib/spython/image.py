import math as _math
from collections.abc import Callable as _Callable
from typing import Any as _Any

from spython import font as _font
from spython.font import Font, FontStyle, FontWeight
from spython.color import Color as _Color
from spython.color import *  # noqa: F401,F403 — re-export all colors
from spython.style import (
    Style,
    fill,
    stroke,
    join,
    none,
    outline_offset as _outline_offset,
    join as _join,
    stroke as _stroke,
    none as _style_none,
)
from spython import system as _system

# Alignment constants
LEFT: str = "left"
CENTER: str = "center"
RIGHT: str = "right"
TOP: str = "top"
MIDDLE: str = "middle"
BOTTOM: str = "bottom"

_FP_EPSILON: float = 1.0e-10
_SQRT3_2: float = 0.8660254037844386
_clip_counter: int = 0


# **************************
# * Point
# **************************


class Pointf:
    def __init__(self, x: float, y: float) -> None:
        self.x: float = x
        self.y: float = y


class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x: int = x
        self.y: int = y


def _point_translate(p: Pointf, dx: float, dy: float) -> Pointf:
    return Pointf(p.x + dx, p.y + dy)


def _point_rotate(p: Pointf, center: Pointf, angle: float) -> Pointf:
    dx: float = p.x - center.x
    dy: float = p.y - center.y
    return Pointf(
        center.x + dx * _cos_deg(angle) - dy * _sin_deg(angle),
        center.y + dx * _sin_deg(angle) + dy * _cos_deg(angle),
    )


def _point_flip_x(p: Pointf) -> Pointf:
    return Pointf(-p.x, p.y)


def _point_flip_y(p: Pointf) -> Pointf:
    return Pointf(p.x, -p.y)


def _point_to_pointf(p: Point) -> Pointf:
    return Pointf(float(p.x), float(p.y))


def _pointf_to_point(p: Pointf) -> Point:
    return Point(round(p.x), round(p.y))


# **************************
# * Utilities
# **************************


def _positive(n: float) -> float:
    return max(0.0, n)


def _mid(a: float, b: float) -> float:
    return (a - b) / 2.0


def _cos_deg(angle: float) -> float:
    return _math.cos(angle * _math.pi / 180.0)


def _sin_deg(angle: float) -> float:
    return _math.sin(angle * _math.pi / 180.0)


# **************************
# * PathCmd
# **************************


class _MoveTo:
    def __init__(self, p: Pointf) -> None:
        self.p: Pointf = p


class _LineTo:
    def __init__(self, p: Pointf) -> None:
        self.p: Pointf = p


class _QuadTo:
    def __init__(self, control: Pointf, end: Pointf) -> None:
        self.control: Pointf = control
        self.end: Pointf = end


class _CubicTo:
    def __init__(self, c1: Pointf, c2: Pointf, end: Pointf) -> None:
        self.c1: Pointf = c1
        self.c2: Pointf = c2
        self.end: Pointf = end


class _ArcTo:
    def __init__(
        self,
        rx: float,
        ry: float,
        rotation: float,
        large_arc: bool,
        sweep: bool,
        end: Pointf,
    ) -> None:
        self.rx: float = rx
        self.ry: float = ry
        self.rotation: float = rotation
        self.large_arc: bool = large_arc
        self.sweep: bool = sweep
        self.end: Pointf = end


def move_to(x: float, y: float) -> _MoveTo:
    return _MoveTo(Pointf(x, y))


def line_to(x: float, y: float) -> _LineTo:
    return _LineTo(Pointf(x, y))


def quad_to(cx: float, cy: float, x: float, y: float) -> _QuadTo:
    return _QuadTo(Pointf(cx, cy), Pointf(x, y))


def cubic_to(
    c1x: float, c1y: float, c2x: float, c2y: float, x: float, y: float
) -> _CubicTo:
    return _CubicTo(Pointf(c1x, c1y), Pointf(c2x, c2y), Pointf(x, y))


def arc_to(
    rx: float,
    ry: float,
    rotation: float,
    large_arc: bool,
    sweep: bool,
    x: float,
    y: float,
) -> _ArcTo:
    return _ArcTo(rx, ry, rotation, large_arc, sweep, Pointf(x, y))


def pathf(commands: list, closed: bool, *style_args: Style) -> "Image":
    return _fix_position(_Path(_make_style(*style_args), commands, closed))


# **************************
# * Image
# **************************


class Image:
    pass


class _Box:
    def __init__(
        self, center: Pointf, width: float, height: float, angle: float
    ) -> None:
        self.center: Pointf = center
        self.width: float = width
        self.height: float = height
        self.angle: float = angle


def _box_translate(b: _Box, dx: float, dy: float) -> _Box:
    return _Box(_point_translate(b.center, dx, dy), b.width, b.height, b.angle)


def _box_box(b: _Box) -> tuple:
    hw: float = b.width / 2.0
    hh: float = b.height / 2.0
    dx: float = hw * abs(_cos_deg(b.angle)) + hh * abs(_sin_deg(b.angle))
    dy: float = hw * abs(_sin_deg(b.angle)) + hh * abs(_cos_deg(b.angle))
    return (_point_translate(b.center, -dx, -dy), _point_translate(b.center, dx, dy))


def _box_rotate(b: _Box, center: Pointf, angle: float) -> _Box:
    return _Box(
        _point_rotate(b.center, center, angle), b.width, b.height, b.angle + angle
    )


def _box_scale(b: _Box, x_factor: float, y_factor: float) -> _Box:
    c: Pointf = Pointf(b.center.x * x_factor, b.center.y * y_factor)
    return _Box(c, b.width * x_factor, b.height * y_factor, b.angle)


def _box_flip(b: _Box, point_flip: _Callable[[Pointf], Pointf]) -> _Box:
    return _Box(point_flip(b.center), b.width, b.height, -b.angle)


class _Path(Image):
    def __init__(self, style: Style, commands: list, closed: bool) -> None:
        self.style: Style = style
        self.commands: list = commands
        self.closed: bool = closed


class _Combination(Image):
    def __init__(self, a: Image, b: Image) -> None:
        self.a: Image = a
        self.b: Image = b


class _Crop(Image):
    def __init__(self, box: _Box, image: Image) -> None:
        self.box: _Box = box
        self.image: Image = image


class _Text(Image):
    def __init__(
        self,
        style: Style,
        box: _Box,
        text: str,
        flip_vertical: bool,
        flip_horizontal: bool,
        font: Font,
    ) -> None:
        self.style: Style = style
        self.box: _Box = box
        self.text: str = text
        self.flip_vertical: bool = flip_vertical
        self.flip_horizontal: bool = flip_horizontal
        self.font: Font = font


class _Bitmap(Image):
    def __init__(self, box: _Box, data_uri: str) -> None:
        self.box: _Box = box
        self.data_uri: str = data_uri


empty: Image = _Path(_style_none, [], True)


def _make_style(*style_args: Style) -> Style:
    if len(style_args) == 0:
        return _style_none
    if len(style_args) == 1:
        return style_args[0]
    return _join(*style_args)


# **************************
# * Measurement
# **************************


def widthf(img: Image) -> float:
    mn, mx = _box(img)
    return mx.x - mn.x


def width(img: Image) -> int:
    return round(widthf(img))


def heightf(img: Image) -> float:
    mn, mx = _box(img)
    return mx.y - mn.y


def height(img: Image) -> int:
    return round(heightf(img))


def dimensionf(img: Image) -> tuple:
    mn, mx = _box(img)
    return (mx.x - mn.x, mx.y - mn.y)


def dimension(img: Image) -> tuple:
    w, h = dimensionf(img)
    return (round(w), round(h))


def centerf(img: Image) -> Pointf:
    mn, mx = _box(img)
    return Pointf(_mid(mx.x, mn.x), _mid(mx.y, mn.y))


def center(img: Image) -> Point:
    return _pointf_to_point(centerf(img))


# **************************
# * Internal transforms
# **************************


def _translate(img: Image, dx: float, dy: float) -> Image:
    if dx == 0.0 and dy == 0.0:
        return img
    if isinstance(img, _Path):
        return _Path(
            img.style, [_cmd_translate(c, dx, dy) for c in img.commands], img.closed
        )
    if isinstance(img, _Combination):
        return _Combination(_translate(img.a, dx, dy), _translate(img.b, dx, dy))
    if isinstance(img, _Crop):
        return _Crop(_box_translate(img.box, dx, dy), _translate(img.image, dx, dy))
    if isinstance(img, _Text):
        return _Text(
            img.style,
            _box_translate(img.box, dx, dy),
            img.text,
            img.flip_vertical,
            img.flip_horizontal,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(_box_translate(img.box, dx, dy), img.data_uri)
    return img


def _fix_position(img: Image) -> Image:
    mn, _ = _box(img)
    if mn.x == 0.0 and mn.y == 0.0:
        return img
    return _translate(img, -mn.x, -mn.y)


# **************************
# * Bounding box
# **************************


def _box(img: Image) -> tuple:
    if isinstance(img, _Path):
        if len(img.commands) == 0:
            return (Pointf(0.0, 0.0), Pointf(0.0, 0.0))
        first = img.commands[0]
        p: Pointf = _cmd_endpoint(first)
        return _path_box(img.commands, 1, p, p.x, p.y, p.x, p.y)
    if isinstance(img, _Combination):
        amn, amx = _box(img.a)
        bmn, bmx = _box(img.b)
        return (
            Pointf(min(amn.x, bmn.x), min(amn.y, bmn.y)),
            Pointf(max(amx.x, bmx.x), max(amx.y, bmx.y)),
        )
    if isinstance(img, _Crop):
        return _box_box(img.box)
    if isinstance(img, _Text):
        return _box_box(img.box)
    if isinstance(img, _Bitmap):
        return _box_box(img.box)
    return (Pointf(0.0, 0.0), Pointf(0.0, 0.0))


def _path_box(
    commands: list,
    idx: int,
    prev: Pointf,
    min_x: float,
    min_y: float,
    max_x: float,
    max_y: float,
) -> tuple:
    while idx < len(commands):
        cmd = commands[idx]
        min_x, min_y, max_x, max_y = _cmd_box(cmd, prev, min_x, min_y, max_x, max_y)
        prev = _cmd_endpoint(cmd)
        idx += 1
    return (Pointf(min_x, min_y), Pointf(max_x, max_y))


def _cmd_box(
    cmd: _Any, prev: Pointf, min_x: float, min_y: float, max_x: float, max_y: float
) -> tuple:
    if isinstance(cmd, _MoveTo):
        return _update_bounds(cmd.p, min_x, min_y, max_x, max_y)
    if isinstance(cmd, _LineTo):
        return _update_bounds(cmd.p, min_x, min_y, max_x, max_y)
    if isinstance(cmd, _QuadTo):
        return _quad_box(prev, cmd.control, cmd.end, min_x, min_y, max_x, max_y)
    if isinstance(cmd, _CubicTo):
        return _cubic_box(prev, cmd.c1, cmd.c2, cmd.end, min_x, min_y, max_x, max_y)
    if isinstance(cmd, _ArcTo):
        return _arc_box(
            prev,
            cmd.rx,
            cmd.ry,
            cmd.rotation,
            cmd.large_arc,
            cmd.sweep,
            cmd.end,
            min_x,
            min_y,
            max_x,
            max_y,
        )
    return (min_x, min_y, max_x, max_y)


def _update_bounds(
    p: Pointf, min_x: float, min_y: float, max_x: float, max_y: float
) -> tuple:
    return (min(min_x, p.x), min(min_y, p.y), max(max_x, p.x), max(max_y, p.y))


def _quad_box(
    p0: Pointf,
    c: Pointf,
    e: Pointf,
    min_x: float,
    min_y: float,
    max_x: float,
    max_y: float,
) -> tuple:
    min_x, min_y, max_x, max_y = _update_bounds(e, min_x, min_y, max_x, max_y)
    min_x, max_x = _quad_axis_extrema(p0.x, c.x, e.x, min_x, max_x)
    min_y, max_y = _quad_axis_extrema(p0.y, c.y, e.y, min_y, max_y)
    return (min_x, min_y, max_x, max_y)


def _quad_axis_extrema(p0: float, c: float, e: float, mn: float, mx: float) -> tuple:
    denom: float = p0 - 2.0 * c + e
    if denom == 0.0:
        return (mn, mx)
    t: float = (p0 - c) / denom
    if 0.0 < t < 1.0:
        v: float = _quad_at(t, p0, c, e)
        return (min(mn, v), max(mx, v))
    return (mn, mx)


def _quad_at(t: float, p0: float, c: float, e: float) -> float:
    mt: float = 1.0 - t
    return mt * mt * p0 + 2.0 * mt * t * c + t * t * e


def _cubic_box(
    p0: Pointf,
    c1: Pointf,
    c2: Pointf,
    e: Pointf,
    min_x: float,
    min_y: float,
    max_x: float,
    max_y: float,
) -> tuple:
    min_x, min_y, max_x, max_y = _update_bounds(e, min_x, min_y, max_x, max_y)
    min_x, max_x = _cubic_axis_extrema(p0.x, c1.x, c2.x, e.x, min_x, max_x)
    min_y, max_y = _cubic_axis_extrema(p0.y, c1.y, c2.y, e.y, min_y, max_y)
    return (min_x, min_y, max_x, max_y)


def _cubic_axis_extrema(
    p0: float, c1: float, c2: float, e: float, mn: float, mx: float
) -> tuple:
    a: float = -p0 + 3.0 * c1 - 3.0 * c2 + e
    b: float = 2.0 * (p0 - 2.0 * c1 + c2)
    c: float = c1 - p0
    if a == 0.0:
        if b == 0.0:
            return (mn, mx)
        t: float = -c / b
        if 0.0 < t < 1.0:
            v: float = _cubic_at(t, p0, c1, c2, e)
            return (min(mn, v), max(mx, v))
        return (mn, mx)
    disc: float = b * b - 4.0 * a * c
    if disc < 0.0:
        return (mn, mx)
    sq: float = _math.sqrt(disc)
    t1: float = (-b + sq) / (2.0 * a)
    t2: float = (-b - sq) / (2.0 * a)
    if 0.0 < t1 < 1.0:
        v1: float = _cubic_at(t1, p0, c1, c2, e)
        mn = min(mn, v1)
        mx = max(mx, v1)
    if 0.0 < t2 < 1.0:
        v2: float = _cubic_at(t2, p0, c1, c2, e)
        mn = min(mn, v2)
        mx = max(mx, v2)
    return (mn, mx)


def _cubic_at(t: float, p0: float, c1: float, c2: float, e: float) -> float:
    mt: float = 1.0 - t
    return (
        mt * mt * mt * p0
        + 3.0 * mt * mt * t * c1
        + 3.0 * mt * t * t * c2
        + t * t * t * e
    )


def _arc_box(
    p1: Pointf,
    rx: float,
    ry: float,
    phi_deg: float,
    large_arc: bool,
    sweep: bool,
    p2: Pointf,
    min_x: float,
    min_y: float,
    max_x: float,
    max_y: float,
) -> tuple:
    min_x, min_y, max_x, max_y = _update_bounds(p2, min_x, min_y, max_x, max_y)
    rx = abs(rx)
    ry = abs(ry)
    if rx == 0.0 or ry == 0.0:
        return (min_x, min_y, max_x, max_y)
    phi: float = phi_deg * _math.pi / 180.0
    cos_phi: float = _math.cos(phi)
    sin_phi: float = _math.sin(phi)
    dx: float = (p1.x - p2.x) / 2.0
    dy: float = (p1.y - p2.y) / 2.0
    x1p: float = cos_phi * dx + sin_phi * dy
    y1p: float = -sin_phi * dx + cos_phi * dy
    lam: float = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry)
    if lam > 1.0:
        s: float = _math.sqrt(lam)
        rx = rx * s
        ry = ry * s
    num: float = max(0.0, rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p)
    den: float = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    sq: float = 0.0 if den == 0.0 else _math.sqrt(num / den)
    sign: float = -1.0 if large_arc == sweep else 1.0
    cxp: float = sign * sq * rx * y1p / ry
    cyp: float = sign * sq * (-ry) * x1p / rx
    cx: float = cos_phi * cxp - sin_phi * cyp + (p1.x + p2.x) / 2.0
    cy: float = sin_phi * cxp + cos_phi * cyp + (p1.y + p2.y) / 2.0
    theta1: float = _angle_vec(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dtheta_raw: float = _angle_vec(
        (x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry
    )
    if not sweep:
        dtheta: float = dtheta_raw - 2.0 * _math.pi if dtheta_raw > 0.0 else dtheta_raw
    else:
        dtheta = dtheta_raw + 2.0 * _math.pi if dtheta_raw < 0.0 else dtheta_raw
    theta_x: float = _math.atan2(-ry * sin_phi, rx * cos_phi)
    theta_y: float = _math.atan2(ry * cos_phi, rx * sin_phi)
    for k in [-1, 0, 1, 2]:
        theta: float = theta_x + float(k) * _math.pi
        if _angle_in_range(theta, theta1, dtheta):
            px, _ = _ellipse_point(cx, cy, rx, ry, cos_phi, sin_phi, theta)
            min_x = min(min_x, px)
            max_x = max(max_x, px)
    for k in [-1, 0, 1, 2]:
        theta = theta_y + float(k) * _math.pi
        if _angle_in_range(theta, theta1, dtheta):
            _, py = _ellipse_point(cx, cy, rx, ry, cos_phi, sin_phi, theta)
            min_y = min(min_y, py)
            max_y = max(max_y, py)
    return (min_x, min_y, max_x, max_y)


def _angle_in_range(theta: float, theta1: float, dtheta: float) -> bool:
    if dtheta >= 0.0:
        return _normalize_angle(theta - theta1) <= dtheta
    else:
        return _normalize_angle(theta1 - theta) <= abs(dtheta)


def _normalize_angle(a: float) -> float:
    two_pi: float = 2.0 * _math.pi
    while a < 0.0:
        a = a + two_pi
    while a >= two_pi:
        a = a - two_pi
    return a


def _ellipse_point(
    cx: float,
    cy: float,
    rx: float,
    ry: float,
    cos_phi: float,
    sin_phi: float,
    theta: float,
) -> tuple:
    ct: float = _math.cos(theta)
    st: float = _math.sin(theta)
    return (
        cx + rx * ct * cos_phi - ry * st * sin_phi,
        cy + rx * ct * sin_phi + ry * st * cos_phi,
    )


def _angle_vec(ux: float, uy: float, vx: float, vy: float) -> float:
    return _math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)


# **************************
# * PathCmd transforms
# **************************


def _cmd_endpoint(cmd: _Any) -> Pointf:
    if isinstance(cmd, _MoveTo):
        return cmd.p
    if isinstance(cmd, _LineTo):
        return cmd.p
    if isinstance(cmd, _QuadTo):
        return cmd.end
    if isinstance(cmd, _CubicTo):
        return cmd.end
    if isinstance(cmd, _ArcTo):
        return cmd.end
    return Pointf(0.0, 0.0)


def _cmd_translate(cmd: _Any, dx: float, dy: float) -> _Any:
    if isinstance(cmd, _MoveTo):
        return _MoveTo(_point_translate(cmd.p, dx, dy))
    if isinstance(cmd, _LineTo):
        return _LineTo(_point_translate(cmd.p, dx, dy))
    if isinstance(cmd, _QuadTo):
        return _QuadTo(
            _point_translate(cmd.control, dx, dy), _point_translate(cmd.end, dx, dy)
        )
    if isinstance(cmd, _CubicTo):
        return _CubicTo(
            _point_translate(cmd.c1, dx, dy),
            _point_translate(cmd.c2, dx, dy),
            _point_translate(cmd.end, dx, dy),
        )
    if isinstance(cmd, _ArcTo):
        return _ArcTo(
            cmd.rx,
            cmd.ry,
            cmd.rotation,
            cmd.large_arc,
            cmd.sweep,
            _point_translate(cmd.end, dx, dy),
        )
    return cmd


def _cmd_rotate(cmd: _Any, center: Pointf, angle: float) -> _Any:
    if isinstance(cmd, _MoveTo):
        return _MoveTo(_point_rotate(cmd.p, center, angle))
    if isinstance(cmd, _LineTo):
        return _LineTo(_point_rotate(cmd.p, center, angle))
    if isinstance(cmd, _QuadTo):
        return _QuadTo(
            _point_rotate(cmd.control, center, angle),
            _point_rotate(cmd.end, center, angle),
        )
    if isinstance(cmd, _CubicTo):
        return _CubicTo(
            _point_rotate(cmd.c1, center, angle),
            _point_rotate(cmd.c2, center, angle),
            _point_rotate(cmd.end, center, angle),
        )
    if isinstance(cmd, _ArcTo):
        return _ArcTo(
            cmd.rx,
            cmd.ry,
            cmd.rotation + angle,
            cmd.large_arc,
            cmd.sweep,
            _point_rotate(cmd.end, center, angle),
        )
    return cmd


def _cmd_scale(cmd: _Any, xf: float, yf: float) -> _Any:
    if isinstance(cmd, _MoveTo):
        return _MoveTo(Pointf(cmd.p.x * xf, cmd.p.y * yf))
    if isinstance(cmd, _LineTo):
        return _LineTo(Pointf(cmd.p.x * xf, cmd.p.y * yf))
    if isinstance(cmd, _QuadTo):
        return _QuadTo(
            Pointf(cmd.control.x * xf, cmd.control.y * yf),
            Pointf(cmd.end.x * xf, cmd.end.y * yf),
        )
    if isinstance(cmd, _CubicTo):
        return _CubicTo(
            Pointf(cmd.c1.x * xf, cmd.c1.y * yf),
            Pointf(cmd.c2.x * xf, cmd.c2.y * yf),
            Pointf(cmd.end.x * xf, cmd.end.y * yf),
        )
    if isinstance(cmd, _ArcTo):
        return _ArcTo(
            cmd.rx * xf,
            cmd.ry * yf,
            cmd.rotation,
            cmd.large_arc,
            cmd.sweep,
            Pointf(cmd.end.x * xf, cmd.end.y * yf),
        )
    return cmd


def _cmd_flip(cmd: _Any, pf: _Callable[[Pointf], Pointf]) -> _Any:
    if isinstance(cmd, _MoveTo):
        return _MoveTo(pf(cmd.p))
    if isinstance(cmd, _LineTo):
        return _LineTo(pf(cmd.p))
    if isinstance(cmd, _QuadTo):
        return _QuadTo(pf(cmd.control), pf(cmd.end))
    if isinstance(cmd, _CubicTo):
        return _CubicTo(pf(cmd.c1), pf(cmd.c2), pf(cmd.end))
    if isinstance(cmd, _ArcTo):
        return _ArcTo(
            cmd.rx, cmd.ry, -cmd.rotation, cmd.large_arc, not cmd.sweep, pf(cmd.end)
        )
    return cmd


def _points_to_path(points: list, style: Style) -> Image:
    n: int = len(points)
    if n == 0:
        return _Path(style, [], False)
    if n == 1:
        return _Path(style, [_MoveTo(points[0])], False)
    if n == 2:
        return _Path(style, [_MoveTo(points[0]), _LineTo(points[1])], False)
    cmds: list = [_MoveTo(points[0])]
    for i in range(1, n):
        cmds.append(_LineTo(points[i]))
    return _Path(style, cmds, True)


# **************************
# * Align
# **************************


def _x_place_dx(x_place: str, wa: float, wb: float) -> tuple:
    if x_place == LEFT:
        return (0.0, 0.0)
    if x_place == RIGHT:
        wm: float = max(wa, wb)
        return (wm - wa, wm - wb)
    # CENTER
    wm = max(wa, wb)
    return (_mid(wm, wa), _mid(wm, wb))


def _y_place_dy(y_place: str, ha: float, hb: float) -> tuple:
    if y_place == TOP:
        return (0.0, 0.0)
    if y_place == BOTTOM:
        hm: float = max(ha, hb)
        return (hm - ha, hm - hb)
    # MIDDLE
    hm = max(ha, hb)
    return (_mid(hm, ha), _mid(hm, hb))


# **************************
# * Basic images
# **************************


def rectangle(w: float, h: float, *style_args: Style) -> Image:
    w = _positive(float(w))
    h = _positive(float(h))
    s: Style = _make_style(*style_args)
    return _Path(
        s,
        [
            _MoveTo(Pointf(0.0, 0.0)),
            _LineTo(Pointf(w, 0.0)),
            _LineTo(Pointf(w, h)),
            _LineTo(Pointf(0.0, h)),
        ],
        True,
    )


def square(side: float, *style_args: Style) -> Image:
    return rectangle(side, side, *style_args)


def ellipse(w: float, h: float, *style_args: Style) -> Image:
    w = _positive(float(w))
    h = _positive(float(h))
    rx: float = w / 2.0
    ry: float = h / 2.0
    s: Style = _make_style(*style_args)
    return _Path(
        s,
        [
            _MoveTo(Pointf(w, ry)),
            _ArcTo(rx, ry, 0.0, False, True, Pointf(0.0, ry)),
            _ArcTo(rx, ry, 0.0, False, True, Pointf(w, ry)),
        ],
        True,
    )


def circle(radius: float, *style_args: Style) -> Image:
    r: float = float(radius)
    return ellipse(2.0 * r, 2.0 * r, *style_args)


def line(x: float, y: float, *style_args: Style) -> Image:
    s: Style = _make_style(*style_args)
    return _fix_position(
        _Path(
            s, [_MoveTo(Pointf(0.0, 0.0)), _LineTo(Pointf(float(x), float(y)))], False
        )
    )


# **************************
# * Polygons
# **************************


def triangle(side: float, *style_args: Style) -> Image:
    side = _positive(float(side))
    h: float = side * _SQRT3_2
    s: Style = _make_style(*style_args)
    return _points_to_path(
        [Pointf(side / 2.0, 0.0), Pointf(side, h), Pointf(0.0, h)], s
    )


def right_triangle(side1: float, side2: float, *style_args: Style) -> Image:
    side1 = _positive(float(side1))
    side2 = _positive(float(side2))
    s: Style = _make_style(*style_args)
    return _points_to_path(
        [Pointf(0.0, 0.0), Pointf(0.0, side2), Pointf(side1, side2)], s
    )


def isosceles_triangle(side_length: float, angle: float, *style_args: Style) -> Image:
    side_length = _positive(float(side_length))
    angle = float(angle)
    hangle: float = angle / 2.0
    s: Style = _make_style(*style_args)
    return _fix_position(
        _points_to_path(
            [
                Pointf(side_length * _sin_deg(hangle), side_length * _cos_deg(hangle)),
                Pointf(0.0, 0.0),
                Pointf(-side_length * _sin_deg(hangle), side_length * _cos_deg(hangle)),
            ],
            s,
        )
    )


def _triangle_from_sides_angle(
    side_b: float, side_c: float, angle_a: float, style: Style
) -> Image:
    cx: float = side_b * _cos_deg(angle_a)
    cy: float = side_b * _sin_deg(angle_a)
    return _fix_position(
        _points_to_path([Pointf(0.0, 0.0), Pointf(side_c, 0.0), Pointf(cx, cy)], style)
    )


def _solve_side(a: float, b: float, angle_c: float) -> float:
    return _math.sqrt(a * a + b * b - 2.0 * a * b * _cos_deg(angle_c))


def _solve_angle(opposite: float, adj1: float, adj2: float) -> float:
    cos_val: float = max(
        -1.0,
        min(
            1.0, (adj1 * adj1 + adj2 * adj2 - opposite * opposite) / (2.0 * adj1 * adj2)
        ),
    )
    return _math.atan2(_math.sqrt(1.0 - cos_val * cos_val), cos_val) * 180.0 / _math.pi


def triangle_sss(
    side_a: float, side_b: float, side_c: float, *style_args: Style
) -> Image:
    side_a = _positive(float(side_a))
    side_b = _positive(float(side_b))
    side_c = _positive(float(side_c))
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_sas(
    side_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    side_a = _positive(float(side_a))
    side_c = _positive(float(side_c))
    angle_b = float(angle_b)
    side_b: float = _solve_side(side_a, side_c, angle_b)
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_ssa(
    side_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    side_a = _positive(float(side_a))
    side_b = _positive(float(side_b))
    angle_c = float(angle_c)
    side_c: float = _solve_side(side_a, side_b, angle_c)
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_aas(
    angle_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    side_c = _positive(float(side_c))
    angle_a = float(angle_a)
    angle_b = float(angle_b)
    angle_c: float = 180.0 - angle_a - angle_b
    ratio: float = side_c / _sin_deg(angle_c)
    side_b: float = ratio * _sin_deg(angle_b)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_ass(
    angle_a: float, side_b: float, side_c: float, *style_args: Style
) -> Image:
    side_b = _positive(float(side_b))
    side_c = _positive(float(side_c))
    return _triangle_from_sides_angle(
        side_b, side_c, float(angle_a), _make_style(*style_args)
    )


def triangle_asa(
    angle_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    side_b = _positive(float(side_b))
    angle_a = float(angle_a)
    angle_c = float(angle_c)
    angle_b: float = 180.0 - angle_a - angle_c
    ratio: float = side_b / _sin_deg(angle_b)
    side_c: float = ratio * _sin_deg(angle_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_saa(
    side_a: float, angle_b: float, angle_c: float, *style_args: Style
) -> Image:
    side_a = _positive(float(side_a))
    angle_b = float(angle_b)
    angle_c = float(angle_c)
    angle_a: float = 180.0 - angle_b - angle_c
    ratio: float = side_a / _sin_deg(angle_a)
    side_b: float = ratio * _sin_deg(angle_b)
    side_c: float = ratio * _sin_deg(angle_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def rhombus(side_length: float, angle: float, *style_args: Style) -> Image:
    side_length = _positive(float(side_length))
    angle = float(angle)
    h: float = 2.0 * side_length * _cos_deg(angle / 2.0)
    w: float = 2.0 * side_length * _sin_deg(angle / 2.0)
    s: Style = _make_style(*style_args)
    return _points_to_path(
        [
            Pointf(0.0, h / 2.0),
            Pointf(w / 2.0, 0.0),
            Pointf(w, h / 2.0),
            Pointf(w / 2.0, h),
        ],
        s,
    )


def regular_polygon(side_length: float, side_count: int, *style_args: Style) -> Image:
    return star_polygon(side_length, side_count, 1, *style_args)


def polygon(points: list, *style_args: Style) -> Image:
    pts: list = []
    for p in points:
        if isinstance(p, Pointf):
            pts.append(p)
        elif isinstance(p, Point):
            pts.append(_point_to_pointf(p))
        else:
            pts.append(Pointf(float(p[0]), float(p[1])))
    return _fix_position(_points_to_path(pts, _make_style(*style_args)))


def star_polygon(
    side_length: float, side_count: int, step_count: int, *style_args: Style
) -> Image:
    side_count = max(1, side_count)
    scf: float = float(side_count)
    step_count = max(1, step_count)
    radius: float = _positive(float(side_length)) / (2.0 * _sin_deg(180.0 / scf))
    alpha: float = 90.0 + 180.0 / scf if side_count % 2 == 0 else -90.0
    pts: list = []
    for i in range(side_count):
        theta: float = alpha + 360.0 * float(i * step_count % side_count) / scf
        pts.append(Pointf(radius * _cos_deg(theta), radius * _sin_deg(theta)))
    s: Style = _make_style(*style_args)
    return _fix_position(_points_to_path(pts, s))


def star(side_length: float, *style_args: Style) -> Image:
    return star_polygon(side_length, 5, 2, *style_args)


def radial_star(
    point_count: int, inner_radius: float, outer_radius: float, *style_args: Style
) -> Image:
    point_count = max(2, point_count)
    inner_radius = _positive(float(inner_radius))
    outer_radius = _positive(float(outer_radius))
    alpha: float = 90.0 + 180.0 / float(point_count) if point_count % 2 == 0 else -90.0
    pts: list = []
    for i in range(point_count):
        theta1: float = alpha + 360.0 * float(i * 2) / float(2 * point_count)
        theta2: float = alpha + 360.0 * float(i * 2 + 1) / float(2 * point_count)
        pts.append(
            Pointf(outer_radius * _cos_deg(theta1), outer_radius * _sin_deg(theta1))
        )
        pts.append(
            Pointf(inner_radius * _cos_deg(theta2), inner_radius * _sin_deg(theta2))
        )
    s: Style = _make_style(*style_args)
    return _fix_position(_points_to_path(pts, s))


def pulled_regular_polygon(
    side_length: float, side_count: int, pull: float, angle: float, *style_args: Style
) -> Image:
    side_count = max(3, side_count)
    scf: float = float(side_count)
    radius: float = _positive(float(side_length)) / (2.0 * _sin_deg(180.0 / scf))
    alpha: float = 90.0 + 180.0 / scf if side_count % 2 == 0 else -90.0
    vertices: list = []
    for i in range(side_count):
        theta: float = alpha + 360.0 * float(i) / scf
        vertices.append(Pointf(radius * _cos_deg(theta), radius * _sin_deg(theta)))
    if len(vertices) == 0:
        return empty
    first: Pointf = vertices[0]
    edges: list = _pulled_edges(vertices, first, float(pull), float(angle))
    s: Style = _make_style(*style_args)
    return _fix_position(_Path(s, [_MoveTo(first)] + edges, True))


def _pulled_edges(vertices: list, first: Pointf, pull: float, angle: float) -> list:
    result: list = []
    n: int = len(vertices)
    for i in range(n):
        a: Pointf = vertices[i]
        b: Pointf = vertices[i + 1] if i + 1 < n else first
        result.append(_edge_cubic(a, b, pull, angle))
    return result


def _edge_cubic(fr: Pointf, to: Pointf, pull: float, angle: float) -> _CubicTo:
    dx: float = to.x - fr.x
    dy: float = to.y - fr.y
    dist: float = _math.sqrt(dx * dx + dy * dy)
    edge_rad: float = _math.atan2(dy, dx)
    angle_rad: float = angle * _math.pi / 180.0
    c1: Pointf = Pointf(
        fr.x + pull * dist * _math.cos(edge_rad + angle_rad),
        fr.y + pull * dist * _math.sin(edge_rad + angle_rad),
    )
    c2: Pointf = Pointf(
        to.x - pull * dist * _math.cos(edge_rad - angle_rad),
        to.y - pull * dist * _math.sin(edge_rad - angle_rad),
    )
    return _CubicTo(c1, c2, to)


# **************************
# * Wedge
# **************************


def wedge(radius: float, angle: float, *style_args: Style) -> Image:
    return _fix_position(
        _wedge_path(float(radius), float(angle), _make_style(*style_args))
    )


def _wedge_path(radius: float, angle: float, style: Style) -> Image:
    r: float = _positive(radius)
    x1: float = r
    y1: float = 0.0
    x2: float = r * _cos_deg(angle)
    y2: float = -r * _sin_deg(angle)
    large_arc: bool = abs(angle) > 180.0
    sweep_flag: bool = angle < 0.0
    return _Path(
        style,
        [
            _MoveTo(Pointf(0.0, 0.0)),
            _LineTo(Pointf(x1, y1)),
            _ArcTo(r, r, 0.0, large_arc, sweep_flag, Pointf(x2, y2)),
        ],
        True,
    )


# **************************
# * Text
# **************************


def text(
    txt: str, *style_args: Style, size: float | None = None, font: Font | None = None
) -> Image:
    f: Font = font if font is not None else Font()
    if size is not None:
        f = Font(
            family=f.family,
            size=float(size),
            style=f.style,
            weight=f.weight,
            underline=f.underline,
        )
    css: str = _font._to_css(f)
    w: float = _system.text_width(txt, css)
    h: float = _system.text_height(txt, css)
    s: Style = _make_style(*style_args)
    return _Text(s, _Box(Pointf(w / 2.0, h / 2.0), w, h, 0.0), txt, False, False, f)


# **************************
# * Bitmap
# **************************


def bitmap(path: str) -> Image:
    w, h, data_uri = _system.load_bitmap(path)
    return _Bitmap(_Box(Pointf(w / 2.0, h / 2.0), w, h, 0.0), data_uri)


def bitmap_data_uri(data_uri: str, width: float, height: float) -> Image:
    return _Bitmap(
        _Box(Pointf(width / 2.0, height / 2.0), width, height, 0.0), data_uri
    )


# **************************
# * Transformations
# **************************


def rotate(img: Image, angle: float) -> Image:
    return _fix_position(_rotate_around(img, centerf(img), -float(angle)))


def _rotate_around(img: Image, center: Pointf, angle: float) -> Image:
    if isinstance(img, _Path):
        return _Path(
            img.style, [_cmd_rotate(c, center, angle) for c in img.commands], img.closed
        )
    if isinstance(img, _Combination):
        return _Combination(
            _rotate_around(img.a, center, angle), _rotate_around(img.b, center, angle)
        )
    if isinstance(img, _Crop):
        return _Crop(
            _box_rotate(img.box, center, angle),
            _rotate_around(img.image, center, angle),
        )
    if isinstance(img, _Text):
        return _Text(
            img.style,
            _box_rotate(img.box, center, angle),
            img.text,
            img.flip_vertical,
            img.flip_horizontal,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(_box_rotate(img.box, center, angle), img.data_uri)
    return img


def scale(img: Image, factor: float) -> Image:
    return scale_xy(img, factor, factor)


def scale_xy(img: Image, x_factor: float, y_factor: float) -> Image:
    x_factor = _positive(float(x_factor))
    y_factor = _positive(float(y_factor))
    if isinstance(img, _Path):
        return _Path(
            img.style,
            [_cmd_scale(c, x_factor, y_factor) for c in img.commands],
            img.closed,
        )
    if isinstance(img, _Combination):
        return _Combination(
            scale_xy(img.a, x_factor, y_factor), scale_xy(img.b, x_factor, y_factor)
        )
    if isinstance(img, _Crop):
        return _Crop(
            _box_scale(img.box, x_factor, y_factor),
            scale_xy(img.image, x_factor, y_factor),
        )
    if isinstance(img, _Text):
        return _Text(
            img.style,
            _box_scale(img.box, x_factor, y_factor),
            img.text,
            img.flip_vertical,
            img.flip_horizontal,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(_box_scale(img.box, x_factor, y_factor), img.data_uri)
    return img


def flip_horizontal(img: Image) -> Image:
    return _fix_position(_flip(img, _point_flip_x, True, False))


def flip_vertical(img: Image) -> Image:
    return _fix_position(_flip(img, _point_flip_y, False, True))


def _flip(
    img: Image, point_flip: _Callable[[Pointf], Pointf], fh: bool, fv: bool
) -> Image:
    if isinstance(img, _Path):
        return _Path(
            img.style, [_cmd_flip(c, point_flip) for c in img.commands], img.closed
        )
    if isinstance(img, _Combination):
        return _Combination(
            _flip(img.a, point_flip, fh, fv), _flip(img.b, point_flip, fh, fv)
        )
    if isinstance(img, _Crop):
        return _Crop(
            _box_flip(img.box, point_flip), _flip(img.image, point_flip, fh, fv)
        )
    if isinstance(img, _Text):
        new_fh: bool = (not img.flip_horizontal) if fh else img.flip_horizontal
        new_fv: bool = (not img.flip_vertical) if fv else img.flip_vertical
        return _Text(
            img.style,
            _box_flip(img.box, point_flip),
            img.text,
            new_fv,
            new_fh,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(_box_flip(img.box, point_flip), img.data_uri)
    return img


def frame(img: Image) -> Image:
    from spython.color import black as _black

    return color_frame(img, _black)


def color_frame(img: Image, color: _Color) -> Image:
    w: float = widthf(img)
    h: float = heightf(img)
    frame_style: Style = _stroke(color, width=2.0)
    return crop(overlay(rectangle(w, h, frame_style), img), 0.0, 0.0, w, h)


def crop(img: Image, x: float, y: float, w: float, h: float) -> Image:
    w = _positive(float(w))
    h = _positive(float(h))
    return _Crop(
        _Box(Pointf(w / 2.0, h / 2.0), w, h, 0.0), _translate(img, -float(x), -float(y))
    )


def crop_align(img: Image, x_place: str, y_place: str, cw: float, ch: float) -> Image:
    cw = _positive(float(cw))
    ch = _positive(float(ch))
    _, dx = _x_place_dx(x_place, widthf(img), cw)
    _, dy = _y_place_dy(y_place, heightf(img), ch)
    return crop(img, dx, dy, cw, ch)


# **************************
# * Overlaying
# **************************


def combine(images: list[Image], op: _Callable[[Image, Image], Image]) -> Image:
    result: Image = empty
    for img in images:
        result = op(result, img)
    return result


def above(a: Image, b: Image) -> Image:
    return above_align(a, CENTER, b)


def above_align(a: Image, x_place: str, b: Image) -> Image:
    dxa, dxb = _x_place_dx(x_place, widthf(a), widthf(b))
    return _Combination(_translate(a, dxa, 0.0), _translate(b, dxb, heightf(a)))


def beside(a: Image, b: Image) -> Image:
    return beside_align(a, MIDDLE, b)


def beside_align(a: Image, y_place: str, b: Image) -> Image:
    dya, dyb = _y_place_dy(y_place, heightf(a), heightf(b))
    return _Combination(_translate(a, 0.0, dya), _translate(b, widthf(a), dyb))


def overlay(top: Image, bottom: Image) -> Image:
    return overlay_align(top, CENTER, MIDDLE, bottom)


def overlay_align(top: Image, x_place: str, y_place: str, bottom: Image) -> Image:
    dxa, dxb = _x_place_dx(x_place, widthf(top), widthf(bottom))
    dya, dyb = _y_place_dy(y_place, heightf(top), heightf(bottom))
    return _fix_position(
        _Combination(_translate(bottom, dxb, dyb), _translate(top, dxa, dya))
    )


def overlay_offset(top: Image, x: float, y: float, bottom: Image) -> Image:
    return overlay(top, _translate(bottom, float(x), float(y)))


def overlay_align_offset(
    top: Image, x_place: str, y_place: str, x: float, y: float, bottom: Image
) -> Image:
    return overlay_align(top, x_place, y_place, _translate(bottom, float(x), float(y)))


def overlay_xy(top: Image, x: float, y: float, bottom: Image) -> Image:
    return _fix_position(_Combination(_translate(bottom, float(x), float(y)), top))


def underlay(bottom: Image, top: Image) -> Image:
    return overlay(top, bottom)


def underlay_align(bottom: Image, x_place: str, y_place: str, top: Image) -> Image:
    return overlay_align(top, x_place, y_place, bottom)


def underlay_offset(bottom: Image, x: float, y: float, top: Image) -> Image:
    return overlay(_translate(top, float(x), float(y)), bottom)


def underlay_align_offset(
    bottom: Image, x_place: str, y_place: str, x: float, y: float, top: Image
) -> Image:
    return underlay_align(bottom, x_place, y_place, _translate(top, float(x), float(y)))


def underlay_xy(bottom: Image, x: float, y: float, top: Image) -> Image:
    return _fix_position(_Combination(bottom, _translate(top, float(x), float(y))))


# **************************
# * Placing
# **************************


def empty_scene(w: float, h: float) -> Image:
    from spython.color import black as _black

    return empty_scene_color(w, h, _black)


def empty_scene_color(w: float, h: float, color: _Color) -> Image:
    w = float(w)
    h = float(h)
    frame_style: Style = _stroke(color, width=2.0)
    return crop(rectangle(w, h, frame_style), 0.0, 0.0, w, h)


def place_image(scene: Image, x: float, y: float, img: Image) -> Image:
    return place_image_align(scene, x, y, CENTER, MIDDLE, img)


def place_image_align(
    scene: Image, x: float, y: float, x_place: str, y_place: str, img: Image
) -> Image:
    x = float(x)
    y = float(y)
    dx: float = 0.0
    if x_place == CENTER:
        dx = widthf(img) / -2.0
    elif x_place == RIGHT:
        dx = -widthf(img)
    dy: float = 0.0
    if y_place == MIDDLE:
        dy = heightf(img) / -2.0
    elif y_place == BOTTOM:
        dy = -heightf(img)
    return _fix_position(
        crop(
            _Combination(scene, _translate(img, x + dx, y + dy)),
            0.0,
            0.0,
            widthf(scene),
            heightf(scene),
        )
    )


def place_images(scene: Image, positions: list, images: list) -> Image:
    for i in range(min(len(positions), len(images))):
        p = positions[i]
        if isinstance(p, Pointf):
            scene = place_image(scene, p.x, p.y, images[i])
        elif isinstance(p, Point):
            scene = place_image(scene, float(p.x), float(p.y), images[i])
        else:
            scene = place_image(scene, float(p[0]), float(p[1]), images[i])
    return scene


def place_images_align(
    scene: Image, positions: list, x_place: str, y_place: str, images: list
) -> Image:
    for i in range(min(len(positions), len(images))):
        p = positions[i]
        if isinstance(p, Pointf):
            scene = place_image_align(scene, p.x, p.y, x_place, y_place, images[i])
        elif isinstance(p, Point):
            scene = place_image_align(
                scene, float(p.x), float(p.y), x_place, y_place, images[i]
            )
        else:
            scene = place_image_align(
                scene, float(p[0]), float(p[1]), x_place, y_place, images[i]
            )
    return scene


def place_line(
    scene: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _Combination(
                scene,
                _Path(
                    s,
                    [
                        _MoveTo(Pointf(float(x1), float(y1))),
                        _LineTo(Pointf(float(x2), float(y2))),
                    ],
                    False,
                ),
            ),
            0.0,
            0.0,
            widthf(scene),
            heightf(scene),
        )
    )


def place_polygon(scene: Image, points: list, *style_args: Style) -> Image:
    pts: list = []
    for p in points:
        if isinstance(p, Pointf):
            pts.append(p)
        elif isinstance(p, Point):
            pts.append(_point_to_pointf(p))
        else:
            pts.append(Pointf(float(p[0]), float(p[1])))
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _Combination(scene, _points_to_path(pts, s)),
            0.0,
            0.0,
            widthf(scene),
            heightf(scene),
        )
    )


def place_curve(
    scene: Image,
    x1: float,
    y1: float,
    angle1: float,
    pull1: float,
    x2: float,
    y2: float,
    angle2: float,
    pull2: float,
    *style_args: Style,
) -> Image:
    x1 = float(x1)
    y1 = float(y1)
    x2 = float(x2)
    y2 = float(y2)
    c1, c2 = _curve_controls(
        x1, y1, float(angle1), float(pull1), x2, y2, float(angle2), float(pull2)
    )
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _Combination(
                scene,
                _Path(
                    s,
                    [_MoveTo(Pointf(x1, y1)), _CubicTo(c1, c2, Pointf(x2, y2))],
                    False,
                ),
            ),
            0.0,
            0.0,
            widthf(scene),
            heightf(scene),
        )
    )


def place_wedge(
    scene: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _Combination(
                scene,
                _translate(
                    _wedge_path(float(radius), float(angle), s), float(x), float(y)
                ),
            ),
            0.0,
            0.0,
            widthf(scene),
            heightf(scene),
        )
    )


def put_image(scene: Image, x: float, y: float, img: Image) -> Image:
    return place_image(scene, float(x), heightf(scene) - float(y), img)


# **************************
# * Adding
# **************************


def add_line(
    img: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    s: Style = _make_style(*style_args)
    return _fix_position(
        _Combination(
            img,
            _Path(
                s,
                [
                    _MoveTo(Pointf(float(x1), float(y1))),
                    _LineTo(Pointf(float(x2), float(y2))),
                ],
                False,
            ),
        )
    )


def add_polygon(img: Image, points: list, *style_args: Style) -> Image:
    pts: list = []
    for p in points:
        if isinstance(p, Pointf):
            pts.append(p)
        elif isinstance(p, Point):
            pts.append(_point_to_pointf(p))
        else:
            pts.append(Pointf(float(p[0]), float(p[1])))
    s: Style = _make_style(*style_args)
    return _fix_position(_Combination(img, _points_to_path(pts, s)))


def add_curve(
    img: Image,
    x1: float,
    y1: float,
    angle1: float,
    pull1: float,
    x2: float,
    y2: float,
    angle2: float,
    pull2: float,
    *style_args: Style,
) -> Image:
    x1 = float(x1)
    y1 = float(y1)
    x2 = float(x2)
    y2 = float(y2)
    c1, c2 = _curve_controls(
        x1, y1, float(angle1), float(pull1), x2, y2, float(angle2), float(pull2)
    )
    s: Style = _make_style(*style_args)
    return _fix_position(
        _Combination(
            img,
            _Path(
                s, [_MoveTo(Pointf(x1, y1)), _CubicTo(c1, c2, Pointf(x2, y2))], False
            ),
        )
    )


def add_solid_curve(
    img: Image,
    x1: float,
    y1: float,
    angle1: float,
    pull1: float,
    x2: float,
    y2: float,
    angle2: float,
    pull2: float,
    *style_args: Style,
) -> Image:
    x1 = float(x1)
    y1 = float(y1)
    x2 = float(x2)
    y2 = float(y2)
    c1, c2 = _curve_controls(
        x1, y1, float(angle1), float(pull1), x2, y2, float(angle2), float(pull2)
    )
    s: Style = _make_style(*style_args)
    return _fix_position(
        _Combination(
            img,
            _Path(s, [_MoveTo(Pointf(x1, y1)), _CubicTo(c1, c2, Pointf(x2, y2))], True),
        )
    )


def add_wedge(
    img: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    s: Style = _make_style(*style_args)
    return _fix_position(
        _Combination(
            img,
            _translate(_wedge_path(float(radius), float(angle), s), float(x), float(y)),
        )
    )


def _curve_controls(
    x1: float,
    y1: float,
    angle1: float,
    pull1: float,
    x2: float,
    y2: float,
    angle2: float,
    pull2: float,
) -> tuple:
    dist: float = _math.sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
    c1: Pointf = Pointf(
        x1 + pull1 * dist * _cos_deg(angle1), y1 - pull1 * dist * _sin_deg(angle1)
    )
    c2: Pointf = Pointf(
        x2 - pull2 * dist * _cos_deg(angle2), y2 + pull2 * dist * _sin_deg(angle2)
    )
    return (c1, c2)


# **************************
# * SVG
# **************************


def to_svg(img: Image) -> str:
    return (
        "<svg "
        + _attrib("width", float(_math.ceil(round(widthf(img), 6))))
        + _attrib("height", float(_math.ceil(round(heightf(img), 6))))
        + 'xmlns="http://www.w3.org/2000/svg">\n'
        + _to_svg(img, 1)
        + "</svg>"
    )


def _next_clip_id() -> int:
    global _clip_counter
    result: int = _clip_counter
    _clip_counter += 1
    return result


def _to_svg(img: Image, level: int) -> str:
    if isinstance(img, _Path):
        aligned: bool = _outline_offset(img.style) > 0.0
        path_d: str = _commands_to_d(img.commands, aligned)
        if img.closed:
            path_d = path_d + " Z"
        return (
            _indent(level)
            + "<path "
            + _attribs("d", path_d)
            + img.style.to_svg()
            + "/>\n"
        )
    if isinstance(img, _Combination):
        return (
            _indent(level)
            + "<g>\n"
            + _to_svg(img.a, level + 1)
            + _to_svg(img.b, level + 1)
            + _indent(level)
            + "</g>\n"
        )
    if isinstance(img, _Crop):
        clipid: str = "clip" + str(_next_clip_id())
        b = img.box
        rect_svg: str = (
            "<rect "
            + _attrib("x", b.center.x - b.width / 2.0)
            + _attrib("y", b.center.y - b.height / 2.0)
            + _attrib("width", b.width)
            + _attrib("height", b.height)
            + _attribs("transform", _rotate_str(b.angle, b.center))
            + "/>"
        )
        return (
            _indent(level)
            + "<defs><clipPath "
            + _attribs("id", clipid)
            + ">"
            + rect_svg
            + "</clipPath></defs>\n"
            + _indent(level)
            + "<g "
            + _attribs("clip-path", "url(#" + clipid + ")")
            + ">\n"
            + _to_svg(img.image, level + 1)
            + _indent(level)
            + "</g>\n"
        )
    if isinstance(img, _Text):
        b = img.box
        css: str = _font._to_css(img.font)
        original_width: float = _system.text_width(img.text, css)
        original_height: float = _system.text_height(img.text, css)
        x_offset: float = _system.text_x_offset(img.text, css)
        y_offset: float = _system.text_y_offset(img.text, css)
        scale_x: float = (
            b.width / original_width * (-1.0 if img.flip_horizontal else 1.0)
        )
        scale_y: float = (
            b.height / original_height * (-1.0 if img.flip_vertical else 1.0)
        )
        return (
            _indent(level)
            + "<text "
            + _attribs("dominant-baseline", "alphabetic")
            + _attribs("text-anchor", "start")
            + _attrib("x", x_offset)
            + _attrib("y", y_offset)
            + _attribs("font-family", img.font.family)
            + _attrib("font-size", img.font.size)
            + _attribs("font-style", _font._style_to_svg(img.font.style))
            + _attribs("font-weight", _font._weight_to_svg(img.font.weight))
            + (_attribs("text-decoration", "underline") if img.font.underline else "")
            + _attribs(
                "transform",
                _translate_str(b.center.x, b.center.y)
                + " "
                + _rotate_str(b.angle, Pointf(0.0, 0.0))
                + " "
                + _scale_str(scale_x, scale_y),
            )
            + img.style.to_svg()
            + ">"
            + img.text
            + "</text>\n"
        )
    if isinstance(img, _Bitmap):
        b = img.box
        result: str = (
            _indent(level)
            + "<image "
            + _attribs("href", img.data_uri)
            + _attrib("x", b.center.x - b.width / 2.0)
            + _attrib("y", b.center.y - b.height / 2.0)
            + _attrib("width", b.width)
            + _attrib("height", b.height)
        )
        if b.angle != 0.0:
            result = result + _attribs("transform", _rotate_str(b.angle, b.center))
        return result + "/>\n"
    return ""


def _commands_to_d(commands: list, aligned: bool) -> str:
    c = _align if aligned else _fs
    parts: list = []
    for cmd in commands:
        parts.append(_cmd_to_d(cmd, c))
    return " ".join(parts)


def _cmd_to_d(cmd: _Any, c: _Callable[[float], str]) -> str:
    if isinstance(cmd, _MoveTo):
        return "M " + c(cmd.p.x) + " " + c(cmd.p.y)
    if isinstance(cmd, _LineTo):
        return "L " + c(cmd.p.x) + " " + c(cmd.p.y)
    if isinstance(cmd, _QuadTo):
        return (
            "Q "
            + c(cmd.control.x)
            + " "
            + c(cmd.control.y)
            + " "
            + c(cmd.end.x)
            + " "
            + c(cmd.end.y)
        )
    if isinstance(cmd, _CubicTo):
        return (
            "C "
            + c(cmd.c1.x)
            + " "
            + c(cmd.c1.y)
            + " "
            + c(cmd.c2.x)
            + " "
            + c(cmd.c2.y)
            + " "
            + c(cmd.end.x)
            + " "
            + c(cmd.end.y)
        )
    if isinstance(cmd, _ArcTo):
        return (
            "A "
            + _fs(cmd.rx)
            + " "
            + _fs(cmd.ry)
            + " "
            + _fs(cmd.rotation)
            + " "
            + ("1" if cmd.large_arc else "0")
            + " "
            + ("1" if cmd.sweep else "0")
            + " "
            + c(cmd.end.x)
            + " "
            + c(cmd.end.y)
        )
    return ""


def _f(v: float) -> str:
    r: str = f"{v:.6f}".rstrip("0").rstrip(".")
    if r == "-0":
        return "0"
    return r


def _fs(v: float) -> str:
    return _f(v)


def _align(v: float) -> str:
    return _f(_math.floor(v + _FP_EPSILON) + 0.5)


def _rotate_str(angle: float, center: Pointf) -> str:
    return "rotate(" + _f(angle) + " " + _f(center.x) + " " + _f(center.y) + ")"


def _scale_str(sx: float, sy: float) -> str:
    return "scale(" + _f(sx) + "," + _f(sy) + ")"


def _translate_str(x: float, y: float) -> str:
    return "translate(" + _f(x) + "," + _f(y) + ")"


def _indent(level: int) -> str:
    return "  " * level


def _attrib(name: str, value: float) -> str:
    return name + '="' + _f(value) + '" '


def _attribs(name: str, value: str) -> str:
    return name + '="' + value + '" '
