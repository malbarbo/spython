from spython.image import (
    above,
    beside,
    circle,
    crop_align,
    to_svg,
    LEFT,
    RIGHT,
    TOP,
    BOTTOM,
    fill,
    palevioletred,
    lightcoral,
)

print(
    to_svg(
        above(
            beside(
                crop_align(circle(40, fill(palevioletred)), RIGHT, BOTTOM, 40, 40),
                crop_align(circle(40, fill(lightcoral)), LEFT, BOTTOM, 40, 40),
            ),
            beside(
                crop_align(circle(40, fill(lightcoral)), RIGHT, TOP, 40, 40),
                crop_align(circle(40, fill(palevioletred)), LEFT, TOP, 40, 40),
            ),
        )
    )
)
