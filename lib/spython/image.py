import math as _math
from collections.abc import Callable as _Callable
from collections.abc import Sequence as _Sequence
from enum import Enum as _Enum

from spython.font import Font, FontStyle, FontWeight
from spython.color import Color as _Color
from spython.color import *  # noqa: F401,F403 — re-export all colors
from spython.style import (
    Style,
    LineCap,
    LineJoin,
    FillRule,
    fill,
    stroke,
    join,
    none,
    join as _join,
    stroke as _stroke,
    none as _style_none,
    _scale as _style_scale,
)
from spython import system as _system
from spython._check import _angle_below, _at_least, _finite, _non_negative, _positive


class XPlace(_Enum):
    LEFT = "left"
    CENTER = "center"
    RIGHT = "right"


class YPlace(_Enum):
    TOP = "top"
    MIDDLE = "middle"
    BOTTOM = "bottom"


# Alignment shorthands so users can write `LEFT` instead of `XPlace.LEFT`.
LEFT: XPlace = XPlace.LEFT
CENTER: XPlace = XPlace.CENTER
RIGHT: XPlace = XPlace.RIGHT
TOP: YPlace = YPlace.TOP
MIDDLE: YPlace = YPlace.MIDDLE
BOTTOM: YPlace = YPlace.BOTTOM

_FP_EPSILON: float = 1.0e-10
_SQRT3_2: float = 0.8660254037844386


# **************************
# * Point
# **************************


class Point:
    def __init__(self, x: float, y: float) -> None:
        self.x: float = float(x)
        self.y: float = float(y)


def _point_translate(p: Point, dx: float, dy: float) -> Point:
    return Point(p.x + dx, p.y + dy)


def _point_rotate(p: Point, center: Point, angle: float) -> Point:
    dx: float = p.x - center.x
    dy: float = p.y - center.y
    return Point(
        center.x + dx * _cos_deg(angle) - dy * _sin_deg(angle),
        center.y + dx * _sin_deg(angle) + dy * _cos_deg(angle),
    )


def _point_flip_x(p: Point) -> Point:
    return Point(-p.x, p.y)


def _point_flip_y(p: Point) -> Point:
    return Point(p.x, -p.y)


# **************************
# * Utilities
# **************************


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
    def __init__(self, p: Point) -> None:
        self.p: Point = p


class _LineTo:
    def __init__(self, p: Point) -> None:
        self.p: Point = p


class _QuadTo:
    def __init__(self, control: Point, end: Point) -> None:
        self.control: Point = control
        self.end: Point = end


class _CubicTo:
    def __init__(self, c1: Point, c2: Point, end: Point) -> None:
        self.c1: Point = c1
        self.c2: Point = c2
        self.end: Point = end


class _ArcTo:
    def __init__(
        self,
        rx: float,
        ry: float,
        rotation: float,
        large_arc: bool,
        sweep: bool,
        end: Point,
    ) -> None:
        self.rx: float = rx
        self.ry: float = ry
        self.rotation: float = rotation
        self.large_arc: bool = large_arc
        self.sweep: bool = sweep
        self.end: Point = end


_PathCmd = _MoveTo | _LineTo | _QuadTo | _CubicTo | _ArcTo


def move_to(x: float, y: float) -> _MoveTo:
    _finite(x=x, y=y)
    return _MoveTo(Point(x, y))


def line_to(x: float, y: float) -> _LineTo:
    _finite(x=x, y=y)
    return _LineTo(Point(x, y))


def quad_to(cx: float, cy: float, x: float, y: float) -> _QuadTo:
    _finite(cx=cx, cy=cy, x=x, y=y)
    return _QuadTo(Point(cx, cy), Point(x, y))


def cubic_to(
    c1x: float, c1y: float, c2x: float, c2y: float, x: float, y: float
) -> _CubicTo:
    _finite(c1x=c1x, c1y=c1y, c2x=c2x, c2y=c2y, x=x, y=y)
    return _CubicTo(Point(c1x, c1y), Point(c2x, c2y), Point(x, y))


def arc_to(
    rx: float,
    ry: float,
    rotation: float,
    large_arc: bool,
    sweep: bool,
    x: float,
    y: float,
) -> _ArcTo:
    _finite(rx=rx, ry=ry, rotation=rotation, x=x, y=y)
    return _ArcTo(rx, ry, rotation, large_arc, sweep, Point(x, y))


def path(commands: _Sequence[_PathCmd], closed: bool, *style_args: Style) -> "Image":
    return _fix_position(_Path(_make_style(*style_args), list(commands), closed))


# **************************
# * Image
# **************************


class Image:
    pass


class _Box:
    """The image of a rectangle under an affine map. `x_axis` goes from the
    middle of the left side to the middle of the right side, and `y_axis`
    from the middle of the top side to the middle of the bottom side. A flip
    makes the two axes a mirror of a rotation."""

    def __init__(self, center: Point, x_axis: Point, y_axis: Point) -> None:
        self.center: Point = center
        self.x_axis: Point = x_axis
        self.y_axis: Point = y_axis


def _box_of_size(w: float, h: float) -> _Box:
    return _Box(Point(w / 2.0, h / 2.0), Point(w, 0.0), Point(0.0, h))


def _box_translate(b: _Box, dx: float, dy: float) -> _Box:
    return _Box(_point_translate(b.center, dx, dy), b.x_axis, b.y_axis)


def _box_box(b: _Box) -> tuple[Point, Point]:
    dx: float = (abs(b.x_axis.x) + abs(b.y_axis.x)) / 2.0
    dy: float = (abs(b.x_axis.y) + abs(b.y_axis.y)) / 2.0
    return (_point_translate(b.center, -dx, -dy), _point_translate(b.center, dx, dy))


def _box_rotate(b: _Box, center: Point, angle: float) -> _Box:
    origin: Point = Point(0.0, 0.0)
    return _Box(
        _point_rotate(b.center, center, angle),
        _point_rotate(b.x_axis, origin, angle),
        _point_rotate(b.y_axis, origin, angle),
    )


def _box_scale(b: _Box, x_factor: float, y_factor: float) -> _Box:
    c: Point = Point(b.center.x * x_factor, b.center.y * y_factor)
    return _Box(
        c,
        Point(b.x_axis.x * x_factor, b.x_axis.y * y_factor),
        Point(b.y_axis.x * x_factor, b.y_axis.y * y_factor),
    )


def _box_flip(b: _Box, point_flip: _Callable[[Point], Point]) -> _Box:
    return _Box(point_flip(b.center), point_flip(b.x_axis), point_flip(b.y_axis))


class _Path(Image):
    def __init__(self, style: Style, commands: list[_PathCmd], closed: bool) -> None:
        self.style: Style = style
        self.commands: list[_PathCmd] = commands
        self.closed: bool = closed


class _Line(Image):
    def __init__(self, style: Style, start: Point, end: Point) -> None:
        self.style: Style = style
        self.start: Point = start
        self.end: Point = end


class _Combination(Image):
    def __init__(self, drawn: tuple[Point, Point] | None, a: Image, b: Image) -> None:
        self.drawn: tuple[Point, Point] | None = drawn
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
        font: Font,
    ) -> None:
        self.style: Style = style
        self.box: _Box = box
        self.text: str = text
        self.font: Font = font


# A bitmap keeps the decoded image, so the scene draws it with no new decode.
class _Bitmap(Image):
    def __init__(self, box: _Box, loaded: object) -> None:
        self.box: _Box = box
        self.loaded: object = loaded


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


def width(img: Image) -> float:
    mn, mx = _box(img)
    return mx.x - mn.x


def height(img: Image) -> float:
    mn, mx = _box(img)
    return mx.y - mn.y


def dimension(img: Image) -> tuple[float, float]:
    mn, mx = _box(img)
    return (mx.x - mn.x, mx.y - mn.y)


def center(img: Image) -> Point:
    mn, mx = _box(img)
    return Point(_mid(mx.x, mn.x), _mid(mx.y, mn.y))


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
    if isinstance(img, _Line):
        return _Line(
            img.style,
            _point_translate(img.start, dx, dy),
            _point_translate(img.end, dx, dy),
        )
    if isinstance(img, _Combination):
        drawn = img.drawn
        if drawn is not None:
            drawn = (
                _point_translate(drawn[0], dx, dy),
                _point_translate(drawn[1], dx, dy),
            )
        return _Combination(drawn, _translate(img.a, dx, dy), _translate(img.b, dx, dy))
    if isinstance(img, _Crop):
        return _Crop(_box_translate(img.box, dx, dy), _translate(img.image, dx, dy))
    if isinstance(img, _Text):
        return _Text(
            img.style,
            _box_translate(img.box, dx, dy),
            img.text,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(
            _box_translate(img.box, dx, dy),
            img.loaded,
        )
    return img


def _fix_position(img: Image) -> Image:
    mn, _ = _box(img)
    if mn.x == 0.0 and mn.y == 0.0:
        return img
    return _translate(img, -mn.x, -mn.y)


# **************************
# * Bounding box
# **************************


def _box(img: Image) -> tuple[Point, Point]:
    box = _drawn_box(img)
    if box is None:
        return (Point(0.0, 0.0), Point(0.0, 0.0))
    return box


def _drawn_box(img: Image) -> tuple[Point, Point] | None:
    """The box of what `img` draws, or `None` when `img` is made only of
    paths that draw nothing. A part with nothing in it adds nothing to the box
    of a combination, wherever a translation left it."""
    if isinstance(img, _Path):
        box = _system.path_box(img.commands)
        if box is None:
            return None
        x0, y0, x1, y1 = box
        return (Point(x0, y0), Point(x1, y1))
    if isinstance(img, _Line):
        # Half a pixel on each side is what a pen of width 1 covers, so the
        # ends sit on the centers of pixels whatever the transformation.
        start, end = img.start, img.end
        return (
            Point(min(start.x, end.x) - 0.5, min(start.y, end.y) - 0.5),
            Point(max(start.x, end.x) + 0.5, max(start.y, end.y) + 0.5),
        )
    if isinstance(img, _Combination):
        return img.drawn
    if isinstance(img, _Crop):
        return _box_box(img.box)
    if isinstance(img, _Text):
        return _box_box(img.box)
    if isinstance(img, _Bitmap):
        return _box_box(img.box)
    return None


def _combination(a: Image, b: Image) -> Image:
    """Draws `b` over `a`. The node keeps the box of the two, so the box of
    a combination does not walk the tree under it."""
    amb = _drawn_box(a)
    bmb = _drawn_box(b)
    if amb is None or bmb is None:
        return _Combination(bmb if amb is None else amb, a, b)
    (amn, amx), (bmn, bmx) = amb, bmb
    drawn = (
        Point(min(amn.x, bmn.x), min(amn.y, bmn.y)),
        Point(max(amx.x, bmx.x), max(amx.y, bmx.y)),
    )
    return _Combination(drawn, a, b)


# **************************
# * PathCmd transforms
# **************************


def _cmd_translate(cmd: _PathCmd, dx: float, dy: float) -> _PathCmd:
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


def _cmd_rotate(cmd: _PathCmd, center: Point, angle: float) -> _PathCmd:
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


def _cmd_scale(cmd: _PathCmd, xf: float, yf: float) -> _PathCmd:
    if isinstance(cmd, _MoveTo):
        return _MoveTo(Point(cmd.p.x * xf, cmd.p.y * yf))
    if isinstance(cmd, _LineTo):
        return _LineTo(Point(cmd.p.x * xf, cmd.p.y * yf))
    if isinstance(cmd, _QuadTo):
        return _QuadTo(
            Point(cmd.control.x * xf, cmd.control.y * yf),
            Point(cmd.end.x * xf, cmd.end.y * yf),
        )
    if isinstance(cmd, _CubicTo):
        return _CubicTo(
            Point(cmd.c1.x * xf, cmd.c1.y * yf),
            Point(cmd.c2.x * xf, cmd.c2.y * yf),
            Point(cmd.end.x * xf, cmd.end.y * yf),
        )
    if isinstance(cmd, _ArcTo):
        return _arc_scale(cmd, xf, yf)
    return cmd


def _arc_scale(cmd: _ArcTo, xf: float, yf: float) -> _ArcTo:
    # A rotated ellipse scaled along x and y is an ellipse with other axes.
    # The axes are the eigenvectors of A = M M^T, where M takes the unit
    # circle to the scaled ellipse.
    ux: float = cmd.rx * _cos_deg(cmd.rotation) * xf
    uy: float = cmd.rx * _sin_deg(cmd.rotation) * yf
    vx: float = -cmd.ry * _sin_deg(cmd.rotation) * xf
    vy: float = cmd.ry * _cos_deg(cmd.rotation) * yf
    a: float = ux * ux + vx * vx
    b: float = ux * uy + vx * vy
    d: float = uy * uy + vy * vy
    rotation: float = _math.degrees(0.5 * _math.atan2(2.0 * b, a - d))
    # Of the four axis directions, take the one closest to the old rotation,
    # so an arc that is not rotated stays that way.
    rotation += 90.0 * round((cmd.rotation - rotation) / 90.0)

    def radius(angle: float) -> float:
        c: float = _cos_deg(angle)
        s: float = _sin_deg(angle)
        return _math.sqrt(max(a * c * c + 2.0 * b * s * c + d * s * s, 0.0))

    return _ArcTo(
        radius(rotation),
        radius(rotation + 90.0),
        rotation,
        cmd.large_arc,
        cmd.sweep,
        Point(cmd.end.x * xf, cmd.end.y * yf),
    )


def _cmd_flip(cmd: _PathCmd, pf: _Callable[[Point], Point]) -> _PathCmd:
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


def _points_to_path(points: list[Point], style: Style) -> Image:
    n: int = len(points)
    if n == 0:
        return _Path(style, [], False)
    if n == 1:
        return _Path(style, [_MoveTo(points[0])], False)
    if n == 2:
        return _Path(style, [_MoveTo(points[0]), _LineTo(points[1])], False)
    cmds: list[_PathCmd] = [_MoveTo(points[0])]
    for i in range(1, n):
        cmds.append(_LineTo(points[i]))
    return _Path(style, cmds, True)


# **************************
# * Align
# **************************


def _x_place_dx(x_place: XPlace, wa: float, wb: float) -> tuple[float, float]:
    if x_place == LEFT:
        return (0.0, 0.0)
    if x_place == RIGHT:
        wm: float = max(wa, wb)
        return (wm - wa, wm - wb)
    # CENTER
    wm = max(wa, wb)
    return (_mid(wm, wa), _mid(wm, wb))


def _y_place_dy(y_place: YPlace, ha: float, hb: float) -> tuple[float, float]:
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
    _non_negative(w=w, h=h)
    s: Style = _make_style(*style_args)
    return _Path(
        s,
        [
            _MoveTo(Point(0.0, 0.0)),
            _LineTo(Point(w, 0.0)),
            _LineTo(Point(w, h)),
            _LineTo(Point(0.0, h)),
        ],
        True,
    )


def square(side: float, *style_args: Style) -> Image:
    _non_negative(side=side)
    return rectangle(side, side, *style_args)


def ellipse(w: float, h: float, *style_args: Style) -> Image:
    _non_negative(w=w, h=h)
    rx: float = w / 2.0
    ry: float = h / 2.0
    s: Style = _make_style(*style_args)

    def arc(end: Point) -> _ArcTo:
        return _ArcTo(rx, ry, 0.0, False, True, end)

    # A quarter arc ends away from where it starts even when the width or the
    # height is 0, so the box keeps the other side.
    return _Path(
        s,
        [
            _MoveTo(Point(w, ry)),
            arc(Point(rx, h)),
            arc(Point(0.0, ry)),
            arc(Point(rx, 0.0)),
            arc(Point(w, ry)),
        ],
        True,
    )


def circle(radius: float, *style_args: Style) -> Image:
    _non_negative(radius=radius)
    return ellipse(2.0 * radius, 2.0 * radius, *style_args)


def line(x: float, y: float, *style_args: Style) -> Image:
    # As in 2htdp, a line takes one more column and one more row than the
    # distance between its ends, so a thin line fits on the canvas.
    _finite(x=x, y=y)
    s: Style = _make_style(*style_args)
    return _fix_position(_Line(s, Point(0.0, 0.0), Point(x, y)))


# **************************
# * Polygons
# **************************


def triangle(side: float, *style_args: Style) -> Image:
    _non_negative(side=side)
    h: float = side * _SQRT3_2
    s: Style = _make_style(*style_args)
    return _points_to_path([Point(side / 2.0, 0.0), Point(side, h), Point(0.0, h)], s)


def right_triangle(side1: float, side2: float, *style_args: Style) -> Image:
    _non_negative(side1=side1, side2=side2)
    s: Style = _make_style(*style_args)
    return _points_to_path([Point(0.0, 0.0), Point(0.0, side2), Point(side1, side2)], s)


def isosceles_triangle(side_length: float, angle: float, *style_args: Style) -> Image:
    _non_negative(side_length=side_length)
    _angle_below(360.0, angle=angle)
    hangle: float = angle / 2.0
    s: Style = _make_style(*style_args)
    return _fix_position(
        _points_to_path(
            [
                Point(side_length * _sin_deg(hangle), side_length * _cos_deg(hangle)),
                Point(0.0, 0.0),
                Point(-side_length * _sin_deg(hangle), side_length * _cos_deg(hangle)),
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
        _points_to_path([Point(0.0, 0.0), Point(side_c, 0.0), Point(cx, cy)], style)
    )


def _two_angles(**angles: float) -> None:
    """Raises `ValueError` if two angles of a triangle leave no positive third
    angle."""
    _angle_below(180.0, **angles)
    (name1, angle1), (name2, angle2) = angles.items()
    if angle1 + angle2 >= 180:
        raise ValueError(
            f"{name1} + {name2} must be less than 180, got {angle1 + angle2}"
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
    _non_negative(side_a=side_a, side_b=side_b, side_c=side_c)
    # Sides where one equals the sum of the other two make a flat triangle.
    if side_a > side_b + side_c or side_b > side_a + side_c or side_c > side_a + side_b:
        raise ValueError(
            "no side can be longer than the sum of the other two, "
            f"got {side_a}, {side_b} and {side_c}"
        )
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_sas(
    side_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    _non_negative(side_a=side_a, side_c=side_c)
    _angle_below(180.0, angle_b=angle_b)
    side_b: float = _solve_side(side_a, side_c, angle_b)
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_ssa(
    side_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    _non_negative(side_a=side_a, side_b=side_b)
    _angle_below(180.0, angle_c=angle_c)
    side_c: float = _solve_side(side_a, side_b, angle_c)
    angle_a: float = _solve_angle(side_a, side_b, side_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_aas(
    angle_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    _non_negative(side_c=side_c)
    _two_angles(angle_a=angle_a, angle_b=angle_b)
    angle_c: float = 180.0 - angle_a - angle_b
    ratio: float = side_c / _sin_deg(angle_c)
    side_b: float = ratio * _sin_deg(angle_b)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_ass(
    angle_a: float, side_b: float, side_c: float, *style_args: Style
) -> Image:
    _non_negative(side_b=side_b, side_c=side_c)
    _angle_below(180.0, angle_a=angle_a)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_asa(
    angle_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    _non_negative(side_b=side_b)
    _two_angles(angle_a=angle_a, angle_c=angle_c)
    angle_b: float = 180.0 - angle_a - angle_c
    ratio: float = side_b / _sin_deg(angle_b)
    side_c: float = ratio * _sin_deg(angle_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def triangle_saa(
    side_a: float, angle_b: float, angle_c: float, *style_args: Style
) -> Image:
    _non_negative(side_a=side_a)
    _two_angles(angle_b=angle_b, angle_c=angle_c)
    angle_a: float = 180.0 - angle_b - angle_c
    ratio: float = side_a / _sin_deg(angle_a)
    side_b: float = ratio * _sin_deg(angle_b)
    side_c: float = ratio * _sin_deg(angle_c)
    return _triangle_from_sides_angle(side_b, side_c, angle_a, _make_style(*style_args))


def rhombus(side_length: float, angle: float, *style_args: Style) -> Image:
    _non_negative(side_length=side_length)
    _angle_below(180.0, angle=angle)
    h: float = 2.0 * side_length * _cos_deg(angle / 2.0)
    w: float = 2.0 * side_length * _sin_deg(angle / 2.0)
    s: Style = _make_style(*style_args)
    return _points_to_path(
        [
            Point(0.0, h / 2.0),
            Point(w / 2.0, 0.0),
            Point(w, h / 2.0),
            Point(w / 2.0, h),
        ],
        s,
    )


def regular_polygon(side_length: float, side_count: int, *style_args: Style) -> Image:
    return star_polygon(side_length, side_count, 1, *style_args)


def polygon(
    points: _Sequence[Point | tuple[float, float]], *style_args: Style
) -> Image:
    pts: list[Point] = []
    for p in points:
        if isinstance(p, Point):
            pts.append(p)
        else:
            pts.append(Point(p[0], p[1]))
        _finite(x=pts[-1].x, y=pts[-1].y)
    return _fix_position(_points_to_path(pts, _make_style(*style_args)))


def star_polygon(
    side_length: float, side_count: int, step_count: int, *style_args: Style
) -> Image:
    _non_negative(side_length=side_length)
    _at_least(3, side_count=side_count)
    _at_least(1, step_count=step_count)
    # Counts with a common divisor visit only some of the vertices.
    if _math.gcd(side_count, step_count) != 1:
        raise ValueError(
            "side_count and step_count must have no common divisor, "
            f"got {side_count} and {step_count}"
        )
    radius: float = side_length / (2.0 * _sin_deg(180.0 / side_count))
    alpha: float = 90.0 + 180.0 / side_count if side_count % 2 == 0 else -90.0
    pts: list[Point] = []
    for i in range(side_count):
        theta: float = alpha + 360.0 * (i * step_count % side_count) / side_count
        pts.append(Point(radius * _cos_deg(theta), radius * _sin_deg(theta)))
    s: Style = _make_style(*style_args)
    return _fix_position(_points_to_path(pts, s))


def star(side_length: float, *style_args: Style) -> Image:
    return star_polygon(side_length, 5, 2, *style_args)


def radial_star(
    point_count: int, inner_radius: float, outer_radius: float, *style_args: Style
) -> Image:
    _at_least(2, point_count=point_count)
    _non_negative(inner_radius=inner_radius, outer_radius=outer_radius)
    alpha: float = 90.0 + 180.0 / point_count if point_count % 2 == 0 else -90.0
    pts: list[Point] = []
    for i in range(point_count):
        theta1: float = alpha + 360.0 * (i * 2) / (2 * point_count)
        theta2: float = alpha + 360.0 * (i * 2 + 1) / (2 * point_count)
        pts.append(
            Point(outer_radius * _cos_deg(theta1), outer_radius * _sin_deg(theta1))
        )
        pts.append(
            Point(inner_radius * _cos_deg(theta2), inner_radius * _sin_deg(theta2))
        )
    s: Style = _make_style(*style_args)
    return _fix_position(_points_to_path(pts, s))


def pulled_regular_polygon(
    side_length: float, side_count: int, pull: float, angle: float, *style_args: Style
) -> Image:
    _non_negative(side_length=side_length)
    _at_least(3, side_count=side_count)
    radius: float = side_length / (2.0 * _sin_deg(180.0 / side_count))
    alpha: float = 90.0 + 180.0 / side_count if side_count % 2 == 0 else -90.0
    vertices: list[Point] = []
    for i in range(side_count):
        theta: float = alpha + 360.0 * i / side_count
        vertices.append(Point(radius * _cos_deg(theta), radius * _sin_deg(theta)))
    first: Point = vertices[0]
    edges: list[_CubicTo] = _pulled_edges(vertices, first, pull, angle)
    s: Style = _make_style(*style_args)
    return _fix_position(_Path(s, [_MoveTo(first)] + edges, True))


def _pulled_edges(
    vertices: list[Point], first: Point, pull: float, angle: float
) -> list[_CubicTo]:
    result: list[_CubicTo] = []
    n: int = len(vertices)
    for i in range(n):
        a: Point = vertices[i]
        b: Point = vertices[i + 1] if i + 1 < n else first
        result.append(_edge_cubic(a, b, pull, angle))
    return result


def _edge_cubic(fr: Point, to: Point, pull: float, angle: float) -> _CubicTo:
    dx: float = to.x - fr.x
    dy: float = to.y - fr.y
    dist: float = _math.sqrt(dx * dx + dy * dy)
    edge_rad: float = _math.atan2(dy, dx)
    angle_rad: float = angle * _math.pi / 180.0
    c1: Point = Point(
        fr.x + pull * dist * _math.cos(edge_rad + angle_rad),
        fr.y + pull * dist * _math.sin(edge_rad + angle_rad),
    )
    c2: Point = Point(
        to.x - pull * dist * _math.cos(edge_rad - angle_rad),
        to.y - pull * dist * _math.sin(edge_rad - angle_rad),
    )
    return _CubicTo(c1, c2, to)


# **************************
# * Wedge
# **************************


def wedge(radius: float, angle: float, *style_args: Style) -> Image:
    return _fix_position(_wedge_path(radius, angle, _make_style(*style_args)))


def _wedge_path(radius: float, angle: float, style: Style) -> Image:
    _non_negative(radius=radius)
    # A wedge of 360 is the whole circle.
    if not 0 < angle <= 360:
        raise ValueError(f"angle must be greater than 0 and at most 360, got {angle}")
    r: float = radius
    arcs: list[_PathCmd]
    if angle == 360:
        # An arc that ends where it starts draws nothing, so a full turn takes
        # two halves.
        arcs = [
            _ArcTo(r, r, 0.0, False, False, Point(-r, 0.0)),
            _ArcTo(r, r, 0.0, False, False, Point(r, 0.0)),
        ]
    else:
        end: Point = Point(r * _cos_deg(angle), -r * _sin_deg(angle))
        arcs = [_ArcTo(r, r, 0.0, angle > 180.0, False, end)]
    return _Path(style, [_MoveTo(Point(0.0, 0.0)), _LineTo(Point(r, 0.0))] + arcs, True)


# **************************
# * Text
# **************************


def text(
    txt: str, *style_args: Style, size: float | None = None, font: Font | None = None
) -> Image:
    if txt == "":
        return empty
    f: Font = font if font is not None else Font()
    if size is not None:
        f = Font(
            family=f.family,
            size=size,
            style=f.style,
            weight=f.weight,
            underline=f.underline,
        )
    w, h = _system.text_metrics(txt, f)
    s: Style = _make_style(*style_args)
    return _Text(s, _box_of_size(w, h), txt, f)


# **************************
# * Bitmap
# **************************


def bitmap_data_uri(data_uri: str) -> Image:
    loaded, w, h = _system.load_image(data_uri)
    return _Bitmap(_box_of_size(w, h), loaded)


# **************************
# * Transformations
# **************************


def rotate(img: Image, angle: float) -> Image:
    _finite(angle=angle)
    return _fix_position(_rotate_around(img, center(img), -angle))


def _rotate_around(img: Image, center: Point, angle: float) -> Image:
    if isinstance(img, _Path):
        return _Path(
            img.style, [_cmd_rotate(c, center, angle) for c in img.commands], img.closed
        )
    if isinstance(img, _Line):
        return _Line(
            img.style,
            _point_rotate(img.start, center, angle),
            _point_rotate(img.end, center, angle),
        )
    if isinstance(img, _Combination):
        return _combination(
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
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(
            _box_rotate(img.box, center, angle),
            img.loaded,
        )
    return img


def scale(img: Image, factor: float) -> Image:
    _positive(factor=factor)
    return scale_xy(img, factor, factor)


def scale_xy(img: Image, x_factor: float, y_factor: float) -> Image:
    _positive(x_factor=x_factor, y_factor=y_factor)
    # The half pixel around a line keeps its size, so the scaled line no
    # longer starts at 0.
    return _fix_position(_scale_parts(img, x_factor, y_factor))


def _scale_parts(img: Image, x_factor: float, y_factor: float) -> Image:
    if isinstance(img, _Path):
        # The path takes the scale point by point, but its stroke has one
        # width and one dash, so they take the mean of the two factors, as the
        # width does in 2htdp.
        return _Path(
            _style_scale(img.style, (x_factor + y_factor) / 2.0),
            [_cmd_scale(c, x_factor, y_factor) for c in img.commands],
            img.closed,
        )
    if isinstance(img, _Line):
        return _Line(
            _style_scale(img.style, (x_factor + y_factor) / 2.0),
            Point(img.start.x * x_factor, img.start.y * y_factor),
            Point(img.end.x * x_factor, img.end.y * y_factor),
        )
    if isinstance(img, _Combination):
        return _combination(
            _scale_parts(img.a, x_factor, y_factor),
            _scale_parts(img.b, x_factor, y_factor),
        )
    if isinstance(img, _Crop):
        return _Crop(
            _box_scale(img.box, x_factor, y_factor),
            _scale_parts(img.image, x_factor, y_factor),
        )
    if isinstance(img, _Text):
        # The stroke of a text is as wide as the stroke of a path.
        return _Text(
            _style_scale(img.style, (x_factor + y_factor) / 2.0),
            _box_scale(img.box, x_factor, y_factor),
            img.text,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(
            _box_scale(img.box, x_factor, y_factor),
            img.loaded,
        )
    return img


def flip_horizontal(img: Image) -> Image:
    return _fix_position(_flip(img, _point_flip_x))


def flip_vertical(img: Image) -> Image:
    return _fix_position(_flip(img, _point_flip_y))


def _flip(img: Image, point_flip: _Callable[[Point], Point]) -> Image:
    if isinstance(img, _Path):
        return _Path(
            img.style, [_cmd_flip(c, point_flip) for c in img.commands], img.closed
        )
    if isinstance(img, _Line):
        return _Line(img.style, point_flip(img.start), point_flip(img.end))
    if isinstance(img, _Combination):
        return _combination(_flip(img.a, point_flip), _flip(img.b, point_flip))
    if isinstance(img, _Crop):
        return _Crop(_box_flip(img.box, point_flip), _flip(img.image, point_flip))
    if isinstance(img, _Text):
        return _Text(
            img.style,
            _box_flip(img.box, point_flip),
            img.text,
            img.font,
        )
    if isinstance(img, _Bitmap):
        return _Bitmap(
            _box_flip(img.box, point_flip),
            img.loaded,
        )
    return img


def frame(img: Image) -> Image:
    from spython.color import black as _black

    return color_frame(img, _black)


def color_frame(img: Image, color: _Color) -> Image:
    w: float = width(img)
    h: float = height(img)
    frame_style: Style = _stroke(color, width=2.0)
    return crop(overlay(rectangle(w, h, frame_style), img), 0.0, 0.0, w, h)


def crop(img: Image, x: float, y: float, w: float, h: float) -> Image:
    _finite(x=x, y=y)
    _non_negative(w=w, h=h)
    return _Crop(_box_of_size(w, h), _translate(img, -x, -y))


def crop_align(
    img: Image, x_place: XPlace, y_place: YPlace, cw: float, ch: float
) -> Image:
    _non_negative(cw=cw, ch=ch)
    # The offset is negative when the crop is larger than the image, which
    # puts the image at the right or bottom of the crop.
    dx: float = width(img) - cw
    if x_place == LEFT:
        dx = 0.0
    elif x_place == CENTER:
        dx = dx / 2.0
    dy: float = height(img) - ch
    if y_place == TOP:
        dy = 0.0
    elif y_place == MIDDLE:
        dy = dy / 2.0
    return crop(img, dx, dy, cw, ch)


# **************************
# * Overlaying
# **************************


def combine(images: list[Image], op: _Callable[[Image, Image], Image]) -> Image:
    result: Image = empty
    for img in images:
        result = op(result, img)
    return result


def _above_pair(a: Image, b: Image, x_place: XPlace) -> Image:
    dxa, dxb = _x_place_dx(x_place, width(a), width(b))
    return _combination(_translate(a, dxa, 0.0), _translate(b, dxb, height(a)))


def above(*images: Image, x_place: XPlace = CENTER) -> Image:
    if len(images) < 2:
        raise TypeError("above expects at least 2 images")
    result: Image = images[0]
    for img in images[1:]:
        result = _above_pair(result, img, x_place)
    return result


def _beside_pair(a: Image, b: Image, y_place: YPlace) -> Image:
    dya, dyb = _y_place_dy(y_place, height(a), height(b))
    return _combination(_translate(a, 0.0, dya), _translate(b, width(a), dyb))


def beside(*images: Image, y_place: YPlace = MIDDLE) -> Image:
    if len(images) < 2:
        raise TypeError("beside expects at least 2 images")
    result: Image = images[0]
    for img in images[1:]:
        result = _beside_pair(result, img, y_place)
    return result


def _overlay_aligned(
    top: Image, bottom: Image, x_place: XPlace, y_place: YPlace
) -> Image:
    dxa, dxb = _x_place_dx(x_place, width(top), width(bottom))
    dya, dyb = _y_place_dy(y_place, height(top), height(bottom))
    return _fix_position(
        _combination(_translate(bottom, dxb, dyb), _translate(top, dxa, dya))
    )


def overlay(
    *images: Image,
    x_offset: float = 0.0,
    y_offset: float = 0.0,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    if len(images) < 2:
        raise TypeError("overlay expects at least 2 images")
    _finite(x_offset=x_offset, y_offset=y_offset)
    result: Image = images[0]
    for img in images[1:]:
        result = _overlay_aligned(
            result, _translate(img, x_offset, y_offset), x_place, y_place
        )
    return result


def overlay_xy(top: Image, x: float, y: float, bottom: Image) -> Image:
    _finite(x=x, y=y)
    return _fix_position(_combination(_translate(bottom, x, y), top))


def underlay(
    *images: Image,
    x_offset: float = 0.0,
    y_offset: float = 0.0,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    if len(images) < 2:
        raise TypeError("underlay expects at least 2 images")
    _finite(x_offset=x_offset, y_offset=y_offset)
    result: Image = images[0]
    for img in images[1:]:
        result = _overlay_aligned(
            _translate(img, x_offset, y_offset), result, x_place, y_place
        )
    return result


def underlay_xy(bottom: Image, x: float, y: float, top: Image) -> Image:
    _finite(x=x, y=y)
    return _fix_position(_combination(bottom, _translate(top, x, y)))


# **************************
# * Placing
# **************************


def empty_scene(w: float, h: float) -> Image:
    from spython.color import black as _black

    return empty_scene_color(w, h, _black)


def empty_scene_color(w: float, h: float, color: _Color) -> Image:
    frame_style: Style = _stroke(color, width=2.0)
    return crop(rectangle(w, h, frame_style), 0.0, 0.0, w, h)


def place_image(
    scene: Image,
    x: float,
    y: float,
    img: Image,
    *,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    _finite(x=x, y=y)
    dx: float = 0.0
    if x_place == CENTER:
        dx = width(img) / -2.0
    elif x_place == RIGHT:
        dx = -width(img)
    dy: float = 0.0
    if y_place == MIDDLE:
        dy = height(img) / -2.0
    elif y_place == BOTTOM:
        dy = -height(img)
    return _fix_position(
        crop(
            _combination(scene, _translate(img, x + dx, y + dy)),
            0.0,
            0.0,
            width(scene),
            height(scene),
        )
    )


def place_images(
    scene: Image,
    positions: list[Point | tuple[float, float]],
    images: list[Image],
    *,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    for i in range(min(len(positions), len(images))):
        p = positions[i]
        if isinstance(p, Point):
            scene = place_image(
                scene, p.x, p.y, images[i], x_place=x_place, y_place=y_place
            )
        else:
            scene = place_image(
                scene,
                p[0],
                p[1],
                images[i],
                x_place=x_place,
                y_place=y_place,
            )
    return scene


def place_line(
    scene: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    _finite(x1=x1, y1=y1, x2=x2, y2=y2)
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _combination(
                scene,
                _Path(
                    s,
                    [
                        _MoveTo(Point(x1, y1)),
                        _LineTo(Point(x2, y2)),
                    ],
                    False,
                ),
            ),
            0.0,
            0.0,
            width(scene),
            height(scene),
        )
    )


def place_polygon(
    scene: Image,
    points: list[Point | tuple[float, float]],
    *style_args: Style,
) -> Image:
    pts: list[Point] = []
    for p in points:
        if isinstance(p, Point):
            pts.append(p)
        else:
            pts.append(Point(p[0], p[1]))
        _finite(x=pts[-1].x, y=pts[-1].y)
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _combination(scene, _points_to_path(pts, s)),
            0.0,
            0.0,
            width(scene),
            height(scene),
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
    c1, c2 = _curve_controls(x1, y1, angle1, pull1, x2, y2, angle2, pull2)
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _combination(
                scene,
                _Path(
                    s,
                    [_MoveTo(Point(x1, y1)), _CubicTo(c1, c2, Point(x2, y2))],
                    False,
                ),
            ),
            0.0,
            0.0,
            width(scene),
            height(scene),
        )
    )


def place_wedge(
    scene: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    _finite(x=x, y=y)
    s: Style = _make_style(*style_args)
    return _fix_position(
        crop(
            _combination(
                scene,
                _translate(_wedge_path(radius, angle, s), x, y),
            ),
            0.0,
            0.0,
            width(scene),
            height(scene),
        )
    )


def put_image(scene: Image, x: float, y: float, img: Image) -> Image:
    return place_image(scene, x, height(scene) - y, img)


# **************************
# * Adding
# **************************


def add_line(
    img: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    _finite(x1=x1, y1=y1, x2=x2, y2=y2)
    s: Style = _make_style(*style_args)
    return _fix_position(
        _combination(
            img,
            _Path(
                s,
                [
                    _MoveTo(Point(x1, y1)),
                    _LineTo(Point(x2, y2)),
                ],
                False,
            ),
        )
    )


def add_polygon(
    img: Image,
    points: list[Point | tuple[float, float]],
    *style_args: Style,
) -> Image:
    pts: list[Point] = []
    for p in points:
        if isinstance(p, Point):
            pts.append(p)
        else:
            pts.append(Point(p[0], p[1]))
        _finite(x=pts[-1].x, y=pts[-1].y)
    s: Style = _make_style(*style_args)
    return _fix_position(_combination(img, _points_to_path(pts, s)))


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
    c1, c2 = _curve_controls(x1, y1, angle1, pull1, x2, y2, angle2, pull2)
    s: Style = _make_style(*style_args)
    return _fix_position(
        _combination(
            img,
            _Path(s, [_MoveTo(Point(x1, y1)), _CubicTo(c1, c2, Point(x2, y2))], False),
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
    c1, c2 = _curve_controls(x1, y1, angle1, pull1, x2, y2, angle2, pull2)
    s: Style = _make_style(*style_args)
    return _fix_position(
        _combination(
            img,
            _Path(s, [_MoveTo(Point(x1, y1)), _CubicTo(c1, c2, Point(x2, y2))], True),
        )
    )


def add_wedge(
    img: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    _finite(x=x, y=y)
    s: Style = _make_style(*style_args)
    return _fix_position(
        _combination(
            img,
            _translate(_wedge_path(radius, angle, s), x, y),
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
) -> tuple[Point, Point]:
    _finite(
        x1=x1,
        y1=y1,
        angle1=angle1,
        pull1=pull1,
        x2=x2,
        y2=y2,
        angle2=angle2,
        pull2=pull2,
    )
    dist: float = _math.sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
    c1: Point = Point(
        x1 + pull1 * dist * _cos_deg(angle1), y1 - pull1 * dist * _sin_deg(angle1)
    )
    c2: Point = Point(
        x2 - pull2 * dist * _cos_deg(angle2), y2 + pull2 * dist * _sin_deg(angle2)
    )
    return (c1, c2)


# **************************
# * SVG
# **************************


def to_svg(img: Image) -> str:
    w, h = _canvas_size(img)
    return _system.scene_svg(img, w, h)


def _to_repl_svg(img: Image) -> str:
    w, h = _repl_canvas_size(img)
    return _system.scene_svg(img, w, h)


# The size of `img` in whole pixels. `to_svg` and a display build a scene of
# this size.
def _canvas_size(img: Image) -> tuple[float, float]:
    mn, mx = _box(img)
    return (
        float(_math.ceil(round(mx.x - mn.x, 6))),
        float(_math.ceil(round(mx.y - mn.y, 6))),
    )


# The canvas of `img` in the REPL, with one more column and one more row,
# where a thin outline on the right or bottom edge draws its line. DrRacket
# shows an image the same way.
def _repl_canvas_size(img: Image) -> tuple[float, float]:
    w, h = _canvas_size(img)
    return (w + 1.0, h + 1.0)
