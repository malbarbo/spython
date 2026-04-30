from spython.image import (
    beside,
    rotate,
    square,
    to_svg,
    Image,
    BOTTOM,
    fill,
    black,
)


def koch_curve(n: int) -> Image:
    if n <= 0:
        return square(1, fill(black))
    else:
        smaller: Image = koch_curve(n - 1)
        return beside(
            smaller,
            rotate(smaller, 60),
            rotate(smaller, -60),
            smaller,
            y_place=BOTTOM,
        )


print(to_svg(koch_curve(5)))
