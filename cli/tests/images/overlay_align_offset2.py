from spython.image import (
    circle,
    overlay,
    star_polygon,
    to_svg,
    LEFT,
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
            x_offset=-10,
            y_offset=10,
            x_place=LEFT,
            y_place=BOTTOM,
        )
    )
)
