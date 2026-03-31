from spython.image import (
    LEFT,
    RIGHT,
    TOP,
    BOTTOM,
    rhombus,
    star_polygon,
    to_svg,
    underlay_align_offset,
    fill,
    navy,
    cornflowerblue,
)

star = star_polygon(20, 11, 3, fill(cornflowerblue))
print(
    to_svg(
        underlay_align_offset(
            underlay_align_offset(
                underlay_align_offset(
                    underlay_align_offset(
                        rhombus(120, 90, fill(navy)), LEFT, TOP, 16, 16, star
                    ),
                    RIGHT,
                    TOP,
                    -16,
                    16,
                    star,
                ),
                LEFT,
                BOTTOM,
                16,
                -16,
                star,
            ),
            RIGHT,
            BOTTOM,
            -16,
            -16,
            star,
        )
    )
)
