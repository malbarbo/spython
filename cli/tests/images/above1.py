from spython.image import (
    above,
    ellipse,
    to_svg,
    fill,
    lightgray,
    darkgray,
    dimgray,
    black,
)

print(
    to_svg(
        above(
            above(
                above(
                    ellipse(70, 20, fill(lightgray)), ellipse(50, 20, fill(darkgray))
                ),
                ellipse(30, 20, fill(dimgray)),
            ),
            ellipse(10, 20, fill(black)),
        )
    )
)
