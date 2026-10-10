from typing import Callable

from spython.image import Style, fill, stroke, red


def show(make: Callable[[], Style]) -> None:
    try:
        make()
        print("ok")
    except ValueError as e:
        print(e)


show(lambda: fill(red, opacity=0))
show(lambda: fill(red, opacity=1))
show(lambda: fill(red, opacity=50))
show(lambda: fill(red, opacity=-0.5))
show(lambda: stroke(red, opacity=1.5))
show(lambda: stroke(red, width=0))
show(lambda: stroke(red, width=-2))
show(lambda: stroke(red, dash_array=[0, 4]))
show(lambda: stroke(red, dash_array=[4, -2]))
