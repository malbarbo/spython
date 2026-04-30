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
                        rhombus(120, 90, fill(navy)), 16, 16, LEFT, TOP, star
                    ),
                    -16,
                    16,
                    RIGHT,
                    TOP,
                    star,
                ),
                16,
                -16,
                LEFT,
                BOTTOM,
                star,
            ),
            -16,
            -16,
            RIGHT,
            BOTTOM,
            star,
        )
    )
)
