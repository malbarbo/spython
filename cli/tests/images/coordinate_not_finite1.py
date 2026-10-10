import math
from typing import Callable

from spython.image import (
    Image,
    Point,
    add_curve,
    add_line,
    add_wedge,
    crop,
    line,
    line_to,
    move_to,
    overlay,
    overlay_xy,
    path,
    place_image,
    place_images,
    polygon,
    put_image,
    rotate,
    square,
    underlay_xy,
    width,
    fill,
    red,
)


def show(make: Callable[[], Image]) -> None:
    try:
        print(width(make()))
    except ValueError as e:
        print(e)


sq = square(10)
show(lambda: line(math.nan, 10))
show(lambda: rotate(sq, math.nan))
show(lambda: rotate(sq, math.inf))
show(lambda: crop(sq, -math.inf, 0, 5, 5))
show(lambda: overlay(sq, sq, x_offset=math.nan))
show(lambda: overlay_xy(sq, 0, math.inf, sq))
show(lambda: underlay_xy(sq, math.nan, 0, sq))
show(lambda: place_image(sq, math.nan, 0, sq))
show(lambda: place_images(sq, [(0, 0), (math.inf, 0)], [sq, sq]))
show(lambda: put_image(sq, 0, math.nan, sq))
show(lambda: polygon([(0, 0), (10, math.nan), (0, 10)]))
show(lambda: polygon([Point(0, 0), Point(math.inf, 0), Point(0, 10)]))
show(lambda: path([move_to(0, 0), line_to(math.nan, 10)], False, fill(red)))
show(lambda: add_line(sq, 0, 0, math.inf, 10, fill(red)))
show(lambda: add_curve(sq, 0, 0, 0, math.nan, 10, 10, 0, 0.5, fill(red)))
show(lambda: add_wedge(sq, math.nan, 0, 5, 90, fill(red)))
