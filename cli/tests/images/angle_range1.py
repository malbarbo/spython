from typing import Callable

from spython.image import (
    Image,
    add_wedge,
    empty,
    isosceles_triangle,
    rhombus,
    triangle_ass,
    triangle_sas,
    triangle_ssa,
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


s = fill(black)
show(lambda: rhombus(10, 0, s))
show(lambda: rhombus(10, 180, s))
show(lambda: rhombus(10, 179.5, s))
show(lambda: wedge(10, -90, s))
show(lambda: wedge(10, 0, s))
show(lambda: wedge(10, 360, s))
show(lambda: wedge(10, 360.5, s))
show(lambda: add_wedge(empty, 0, 0, 10, 400, s))
show(lambda: isosceles_triangle(10, 360, s))
show(lambda: isosceles_triangle(10, 300, s))
show(lambda: triangle_sas(10, 180, 10, s))
show(lambda: triangle_ssa(10, 10, -30, s))
show(lambda: triangle_ass(0, 10, 10, s))
