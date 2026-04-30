from spython.image import (
    LEFT,
    MIDDLE,
    ellipse,
    rectangle,
    to_svg,
    underlay,
    fill,
    orange,
    purple,
)

print(
    to_svg(
        underlay(
            rectangle(30, 60, fill(orange)),
            ellipse(60, 30, fill(purple)),
            x_place=LEFT,
            y_place=MIDDLE,
        )
    )
)
