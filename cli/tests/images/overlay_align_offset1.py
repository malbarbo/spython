from spython.image import (
    circle,
    overlay,
    star_polygon,
    to_svg,
    RIGHT,
    BOTTOM,
    fill,
    navy,
    cornflowerblue,
)

print(
    to_svg(
        overlay(
            star_polygon(20, 20, 3, fill(navy)),
            circle(30, fill(cornflowerblue)),
            x_offset=10,
            y_offset=10,
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
