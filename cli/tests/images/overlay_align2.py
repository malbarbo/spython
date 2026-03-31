from spython.image import (
    overlay_align,
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
        overlay_align(
            overlay_align(
                overlay_align(
                    square(20, fill(silver)), RIGHT, BOTTOM, square(30, fill(seagreen))
                ),
                RIGHT,
                BOTTOM,
                square(40, fill(silver)),
            ),
            RIGHT,
            BOTTOM,
            square(50, fill(seagreen)),
        )
    )
)
