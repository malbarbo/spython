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
show(lambda: regular_polygon(1, 100000, s))
show(lambda: regular_polygon(1, 100001, s))
show(lambda: regular_polygon(1, 4000000000, s))
show(lambda: regular_polygon(1, 10**30, s))
show(lambda: star_polygon(1, 100001, 3, s))
show(lambda: star_polygon(1, 7, 4000000001, s))
show(lambda: radial_star(100001, 1, 2, s))
show(lambda: pulled_regular_polygon(1, 100001, 0.5, 10, s))
