from spython.image import (
    beside,
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
        beside(
            beside(
                beside(
                    ellipse(20, 70, fill(lightgray)), ellipse(20, 50, fill(darkgray))
                ),
                ellipse(20, 30, fill(dimgray)),
            ),
            ellipse(20, 10, fill(black)),
        )
    )
)
