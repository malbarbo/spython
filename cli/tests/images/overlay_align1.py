from spython.image import (
    ellipse,
    overlay,
    rectangle,
    to_svg,
    LEFT,
    MIDDLE,
    fill,
    orange,
    purple,
)

print(
    to_svg(
        overlay(
            rectangle(30, 60, fill(orange)),
            ellipse(60, 30, fill(purple)),
            x_place=LEFT,
            y_place=MIDDLE,
        )
    )
)
