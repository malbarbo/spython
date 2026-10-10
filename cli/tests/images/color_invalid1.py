from typing import Callable

from spython.image import Color, rgb, rgba


def show(make: Callable[[], Color]) -> None:
    try:
        make()
        print("ok")
    except ValueError as e:
        print(e)


show(lambda: rgb(0, 128, 255))
show(lambda: rgb(300, 0, 0))
show(lambda: rgb(0, -1, 0))
show(lambda: rgba(0, 0, 256, 0.5))
show(lambda: rgba(0, 0, 0, 1.5))
show(lambda: rgba(0, 0, 0, -0.1))
show(lambda: Color(0, 0, 0, 2.0))
