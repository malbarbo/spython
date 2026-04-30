from spython.image import (
    circle,
    overlay_align_offset,
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
        overlay_align_offset(
            star_polygon(20, 20, 3, fill(navy)),
            10,
            10,
            RIGHT,
            BOTTOM,
            circle(30, fill(cornflowerblue)),
        )
    )
)
