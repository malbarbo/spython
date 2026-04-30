from spython.image import (
    overlay,
    square,
    to_svg,
    RIGHT,
    BOTTOM,
    fill,
    silver,
    seagreen,
)

print(
    to_svg(
        overlay(
            square(20, fill(silver)),
            square(30, fill(seagreen)),
            square(40, fill(silver)),
            square(50, fill(seagreen)),
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
