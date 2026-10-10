from typing import Callable

from spython.image import (
    Image,
    pulled_regular_polygon,
    radial_star,
    regular_polygon,
    star_polygon,
    fill,
    black,
)


def show(make: Callable[[], Image]) -> None:
    try:
        make()
        print("ok")
    except ValueError as e:
        print(e)


s = fill(black)
show(lambda: regular_polygon(10, 2, s))
show(lambda: regular_polygon(10, -3, s))
show(lambda: regular_polygon(10, 3, s))
show(lambda: star_polygon(10, 6, 2, s))
show(lambda: star_polygon(10, 5, 0, s))
show(lambda: star_polygon(10, 5, 5, s))
show(lambda: star_polygon(10, 5, 2, s))
show(lambda: radial_star(1, 5, 10, s))
show(lambda: radial_star(2, 5, 10, s))
show(lambda: pulled_regular_polygon(10, 2, 0.5, 30.0, s))
show(lambda: pulled_regular_polygon(10, 3, 0.5, 30.0, s))
