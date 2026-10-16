from collections.abc import Callable as _Callable
from collections.abc import Sequence as _Sequence
from enum import Enum as _Enum
from typing import TYPE_CHECKING as _TYPE_CHECKING
from typing import Never as _Never

import _spython_image as _native  # type: ignore[import-not-found]

from spython.font import Font, FontStyle, FontWeight
from spython.color import Color as _Color
from spython.color import black as _black
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
)


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


# **************************
# * Point
# **************************


class Point:
    def __init__(self, x: float, y: float) -> None:
        self.x: float = float(x)
        self.y: float = float(y)


# **************************
# * Image
# **************************

# The classes live in `_spython_image`, which ty cannot read, so ty reads
# these declarations instead.
if _TYPE_CHECKING:

    class Image:
        # No value fits `_Never`, so the checker rejects any call to Image.
        def __init__(self, _: _Never) -> None: ...

    class _PathCmd:
        pass

else:
    Image = _native.Image
    _PathCmd = _native._PathCmd


empty: Image = _native.empty()


def _make_style(*style_args: Style) -> Style:
    return _native.join(list(style_args))


# **************************
# * PathCmd
# **************************


def move_to(x: float, y: float) -> _PathCmd:
    return _native.move_to(x, y)


def line_to(x: float, y: float) -> _PathCmd:
    return _native.line_to(x, y)


def quad_to(cx: float, cy: float, x: float, y: float) -> _PathCmd:
    return _native.quad_to(cx, cy, x, y)


def cubic_to(
    c1x: float, c1y: float, c2x: float, c2y: float, x: float, y: float
) -> _PathCmd:
    return _native.cubic_to(c1x, c1y, c2x, c2y, x, y)


def arc_to(
    rx: float,
    ry: float,
    rotation: float,
    large_arc: bool,
    sweep: bool,
    x: float,
    y: float,
) -> _PathCmd:
    return _native.arc_to(rx, ry, rotation, large_arc, sweep, (x, y))


def path(commands: _Sequence[_PathCmd], closed: bool, *style_args: Style) -> Image:
    return _native.path(list(commands), closed, _make_style(*style_args))


# **************************
# * Measurement
# **************************


def width(img: Image) -> float:
    return _native.width(img)


def height(img: Image) -> float:
    return _native.height(img)


def dimension(img: Image) -> tuple[float, float]:
    return (_native.width(img), _native.height(img))


def center(img: Image) -> Point:
    x, y = _native.center(img)
    return Point(x, y)


# **************************
# * Basic images
# **************************


def rectangle(w: float, h: float, *style_args: Style) -> Image:
    return _native.rectangle(w, h, _make_style(*style_args))


def square(side: float, *style_args: Style) -> Image:
    return _native.square(side, _make_style(*style_args))


def ellipse(w: float, h: float, *style_args: Style) -> Image:
    return _native.ellipse(w, h, _make_style(*style_args))


def circle(radius: float, *style_args: Style) -> Image:
    return _native.circle(radius, _make_style(*style_args))


def line(x: float, y: float, *style_args: Style) -> Image:
    return _native.line(x, y, _make_style(*style_args))


# **************************
# * Polygons
# **************************


def triangle(side: float, *style_args: Style) -> Image:
    return _native.triangle(side, _make_style(*style_args))


def right_triangle(side1: float, side2: float, *style_args: Style) -> Image:
    return _native.right_triangle(side1, side2, _make_style(*style_args))


def isosceles_triangle(side_length: float, angle: float, *style_args: Style) -> Image:
    return _native.isosceles_triangle(side_length, angle, _make_style(*style_args))


def triangle_sss(
    side_a: float, side_b: float, side_c: float, *style_args: Style
) -> Image:
    return _native.triangle_sss(side_a, side_b, side_c, _make_style(*style_args))


def triangle_sas(
    side_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    return _native.triangle_sas(side_a, angle_b, side_c, _make_style(*style_args))


def triangle_ssa(
    side_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    return _native.triangle_ssa(side_a, side_b, angle_c, _make_style(*style_args))


def triangle_aas(
    angle_a: float, angle_b: float, side_c: float, *style_args: Style
) -> Image:
    return _native.triangle_aas(angle_a, angle_b, side_c, _make_style(*style_args))


def triangle_ass(
    angle_a: float, side_b: float, side_c: float, *style_args: Style
) -> Image:
    return _native.triangle_ass(angle_a, side_b, side_c, _make_style(*style_args))


def triangle_asa(
    angle_a: float, side_b: float, angle_c: float, *style_args: Style
) -> Image:
    return _native.triangle_asa(angle_a, side_b, angle_c, _make_style(*style_args))


def triangle_saa(
    side_a: float, angle_b: float, angle_c: float, *style_args: Style
) -> Image:
    return _native.triangle_saa(side_a, angle_b, angle_c, _make_style(*style_args))


def rhombus(side_length: float, angle: float, *style_args: Style) -> Image:
    return _native.rhombus(side_length, angle, _make_style(*style_args))


def regular_polygon(side_length: float, side_count: int, *style_args: Style) -> Image:
    return _native.regular_polygon(side_length, side_count, _make_style(*style_args))


def polygon(
    points: _Sequence[Point | tuple[float, float]], *style_args: Style
) -> Image:
    return _native.polygon(list(points), _make_style(*style_args))


def star_polygon(
    side_length: float, side_count: int, step_count: int, *style_args: Style
) -> Image:
    return _native.star_polygon(
        side_length, side_count, step_count, _make_style(*style_args)
    )


def star(side_length: float, *style_args: Style) -> Image:
    return _native.star(side_length, _make_style(*style_args))


def radial_star(
    point_count: int, inner_radius: float, outer_radius: float, *style_args: Style
) -> Image:
    return _native.radial_star(
        point_count, inner_radius, outer_radius, _make_style(*style_args)
    )


def pulled_regular_polygon(
    side_length: float, side_count: int, pull: float, angle: float, *style_args: Style
) -> Image:
    return _native.pulled_regular_polygon(
        side_length, side_count, pull, angle, _make_style(*style_args)
    )


# **************************
# * Wedge
# **************************


def wedge(radius: float, angle: float, *style_args: Style) -> Image:
    return _native.wedge(radius, angle, _make_style(*style_args))


# **************************
# * Text
# **************************


def text(
    txt: str, *style_args: Style, size: float | None = None, font: Font | None = None
) -> Image:
    f: Font = font if font is not None else Font()
    return _native.text(txt, f, size, _make_style(*style_args))


# **************************
# * Bitmap
# **************************


def bitmap_data_uri(data_uri: str) -> Image:
    return _native.bitmap_data_uri(data_uri)


# **************************
# * Transformations
# **************************


def rotate(img: Image, angle: float) -> Image:
    return _native.rotate(img, angle)


def scale(img: Image, factor: float) -> Image:
    return _native.scale(img, factor)


def scale_xy(img: Image, x_factor: float, y_factor: float) -> Image:
    return _native.scale_xy(img, x_factor, y_factor)


def flip_horizontal(img: Image) -> Image:
    return _native.flip_horizontal(img)


def flip_vertical(img: Image) -> Image:
    return _native.flip_vertical(img)


def frame(img: Image) -> Image:
    return _native.frame(img, _black)


def color_frame(img: Image, color: _Color) -> Image:
    return _native.frame(img, color)


def crop(img: Image, x: float, y: float, w: float, h: float) -> Image:
    return _native.crop(img, x, y, w, h)


def crop_align(
    img: Image, x_place: XPlace, y_place: YPlace, cw: float, ch: float
) -> Image:
    return _native.crop_align(img, x_place, y_place, cw, ch)


# **************************
# * Overlaying
# **************************


def combine(images: _Sequence[Image], op: _Callable[[Image, Image], Image]) -> Image:
    result: Image = empty
    for img in images:
        result = op(result, img)
    return result


def above(i1: Image, i2: Image, *images: Image, x_place: XPlace = CENTER) -> Image:
    return _native.above([i1, i2, *images], x_place)


def beside(i1: Image, i2: Image, *images: Image, y_place: YPlace = MIDDLE) -> Image:
    return _native.beside([i1, i2, *images], y_place)


def overlay(
    i1: Image,
    i2: Image,
    *images: Image,
    x_offset: float = 0.0,
    y_offset: float = 0.0,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    return _native.overlay([i1, i2, *images], x_offset, y_offset, x_place, y_place)


def overlay_xy(top: Image, x: float, y: float, bottom: Image) -> Image:
    return _native.overlay_xy(top, x, y, bottom)


def underlay(
    i1: Image,
    i2: Image,
    *images: Image,
    x_offset: float = 0.0,
    y_offset: float = 0.0,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    return _native.underlay([i1, i2, *images], x_offset, y_offset, x_place, y_place)


def underlay_xy(bottom: Image, x: float, y: float, top: Image) -> Image:
    return _native.underlay_xy(bottom, x, y, top)


# **************************
# * Placing
# **************************


def empty_scene(w: float, h: float) -> Image:
    return _native.empty_scene(w, h, _black)


def empty_scene_color(w: float, h: float, color: _Color) -> Image:
    return _native.empty_scene(w, h, color)


def place_image(
    scene: Image,
    x: float,
    y: float,
    img: Image,
    *,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    return _native.place_image(scene, x, y, img, x_place, y_place)


def place_images(
    scene: Image,
    positions: _Sequence[Point | tuple[float, float]],
    images: _Sequence[Image],
    *,
    x_place: XPlace = CENTER,
    y_place: YPlace = MIDDLE,
) -> Image:
    return _native.place_images(scene, list(positions), list(images), x_place, y_place)


def place_line(
    scene: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    return _native.place_line(scene, x1, y1, x2, y2, _make_style(*style_args))


def place_polygon(
    scene: Image,
    points: _Sequence[Point | tuple[float, float]],
    *style_args: Style,
) -> Image:
    return _native.place_polygon(scene, list(points), _make_style(*style_args))


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
    return _native.place_curve(
        scene,
        (x1, y1, angle1, pull1),
        (x2, y2, angle2, pull2),
        _make_style(*style_args),
    )


def place_wedge(
    scene: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    return _native.place_wedge(scene, x, y, radius, angle, _make_style(*style_args))


def put_image(scene: Image, x: float, y: float, img: Image) -> Image:
    return _native.put_image(scene, x, y, img)


# **************************
# * Adding
# **************************


def add_line(
    img: Image, x1: float, y1: float, x2: float, y2: float, *style_args: Style
) -> Image:
    return _native.add_line(img, x1, y1, x2, y2, _make_style(*style_args))


def add_polygon(
    img: Image,
    points: _Sequence[Point | tuple[float, float]],
    *style_args: Style,
) -> Image:
    return _native.add_polygon(img, list(points), _make_style(*style_args))


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
    return _native.add_curve(
        img,
        (x1, y1, angle1, pull1),
        (x2, y2, angle2, pull2),
        _make_style(*style_args),
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
    return _native.add_solid_curve(
        img,
        (x1, y1, angle1, pull1),
        (x2, y2, angle2, pull2),
        _make_style(*style_args),
    )


def add_wedge(
    img: Image, x: float, y: float, radius: float, angle: float, *style_args: Style
) -> Image:
    return _native.add_wedge(img, x, y, radius, angle, _make_style(*style_args))


# **************************
# * SVG
# **************************


def to_svg(img: Image) -> str:
    return _native.to_svg(img)


def _to_repl_svg(img: Image) -> str:
    return _native.to_repl_svg(img)
