from spython.image import (
    beside,
    flip_horizontal,
    rotate,
    square,
    to_svg,
    fill,
    red,
    blue,
)

print(
    to_svg(
        beside(
            rotate(square(50, fill(red)), 30),
            flip_horizontal(rotate(square(50, fill(blue)), 30)),
        )
    )
)
