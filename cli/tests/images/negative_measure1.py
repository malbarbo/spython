from typing import Callable

from spython.image import (
    Image,
    circle,
    crop,
    ellipse,
    empty,
    rectangle,
    scale,
    square,
    star_polygon,
    wedge,
    fill,
    black,
)


def show(make: Callable[[], Image]) -> None:
    try:
        make()
        print("ok")
    except ValueError as e:
        print(e)


show(lambda: rectangle(-10, 20, fill(black)))
show(lambda: rectangle(10, -20.5, fill(black)))
show(lambda: rectangle(0, 0, fill(black)))
show(lambda: square(-1, fill(black)))
show(lambda: circle(-5, fill(black)))
show(lambda: ellipse(10, -1, fill(black)))
show(lambda: star_polygon(-10, 5, 2, fill(black)))
show(lambda: wedge(-10, 90, fill(black)))
show(lambda: crop(empty, -5, -5, 10, -1))
show(lambda: scale(empty, -2))
