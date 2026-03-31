from spython.image import (
    RIGHT,
    TOP,
    square,
    to_svg,
    underlay_align,
    fill,
    seagreen,
    silver,
)

print(
    to_svg(
        underlay_align(
            underlay_align(
                underlay_align(
                    square(50, fill(seagreen)), RIGHT, TOP, square(40, fill(silver))
                ),
                RIGHT,
                TOP,
                square(30, fill(seagreen)),
            ),
            RIGHT,
            TOP,
            square(20, fill(silver)),
        )
    )
)
