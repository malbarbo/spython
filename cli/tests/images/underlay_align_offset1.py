from spython.image import (
    RIGHT,
    BOTTOM,
    circle,
    star_polygon,
    to_svg,
    underlay_align_offset,
    fill,
    navy,
    cornflowerblue,
)

print(
    to_svg(
        underlay_align_offset(
            star_polygon(20, 20, 3, fill(navy)),
            RIGHT,
            BOTTOM,
            10,
            10,
            circle(30, fill(cornflowerblue)),
        )
    )
)
