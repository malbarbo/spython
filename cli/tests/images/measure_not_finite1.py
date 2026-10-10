import math
from typing import Callable

from spython.image import Image, rectangle, scale, square, stroke, text, width, red


def show(make: Callable[[], Image]) -> None:
    try:
        print(width(make()))
    except ValueError as e:
        print(e)


show(lambda: rectangle(math.nan, 10))
show(lambda: rectangle(10, math.inf))
show(lambda: scale(square(10), math.inf))
show(lambda: text("abc", size=math.inf))
show(lambda: square(10, stroke(red, width=math.nan)))
show(lambda: square(10, stroke(red, dash_array=[2, -math.inf])))
