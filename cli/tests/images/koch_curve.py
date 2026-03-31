from spython.image import (
    beside_align,
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
        return beside_align(
            beside_align(
                beside_align(smaller, BOTTOM, rotate(smaller, 60)),
                BOTTOM,
                rotate(smaller, -60),
            ),
            BOTTOM,
            smaller,
        )


print(to_svg(koch_curve(5)))
