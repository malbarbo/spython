from typing import Callable

from spython.image import (
    Image,
    triangle_aas,
    triangle_asa,
    triangle_saa,
    triangle_sss,
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
show(lambda: triangle_sss(1, 1, 10, s))
show(lambda: triangle_sss(10, 1, 1, s))
show(lambda: triangle_sss(1, 1, 2, s))
show(lambda: triangle_aas(100, 100, 10, s))
show(lambda: triangle_aas(90, 90, 10, s))
show(lambda: triangle_aas(-30, 60, 10, s))
show(lambda: triangle_aas(30, 60, 10, s))
show(lambda: triangle_asa(120, 10, 120, s))
show(lambda: triangle_asa(0, 10, 60, s))
show(lambda: triangle_asa(60, 10, 60, s))
show(lambda: triangle_saa(10, 90, 90, s))
show(lambda: triangle_saa(10, 60, 60, s))
