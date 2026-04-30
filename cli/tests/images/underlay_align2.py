from spython.image import (
    RIGHT,
    TOP,
    square,
    to_svg,
    underlay,
    fill,
    seagreen,
    silver,
)

print(
    to_svg(
        underlay(
            square(50, fill(seagreen)),
            square(40, fill(silver)),
            square(30, fill(seagreen)),
            square(20, fill(silver)),
            x_place=RIGHT,
            y_place=TOP,
        )
    )
)
