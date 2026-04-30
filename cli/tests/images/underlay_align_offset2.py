from spython.image import (
    RIGHT,
    BOTTOM,
    circle,
    star_polygon,
    to_svg,
    underlay,
    fill,
    navy,
    cornflowerblue,
)

print(
    to_svg(
        underlay(
            star_polygon(20, 20, 3, fill(navy)),
            circle(30, fill(cornflowerblue)),
            x_offset=10,
            y_offset=10,
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
