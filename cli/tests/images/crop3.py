from spython.image import (
    above,
    beside,
    circle,
    crop,
    rotate,
    to_svg,
    fill,
    palevioletred,
    lightcoral,
)

print(
    to_svg(
        rotate(
            above(
                beside(
                    crop(circle(40, fill(palevioletred)), 40, 40, 40, 40),
                    crop(circle(40, fill(lightcoral)), 0, 40, 40, 40),
                ),
                beside(
                    crop(circle(40, fill(lightcoral)), 40, 0, 40, 40),
                    crop(circle(40, fill(palevioletred)), 0, 0, 40, 40),
                ),
            ),
            30,
        )
    )
)
