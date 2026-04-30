from spython.image import (
    above,
    ellipse,
    to_svg,
    LEFT,
    fill,
    gold,
    goldenrod,
    darkgoldenrod,
    sienna,
)

print(
    to_svg(
        above(
            ellipse(70, 20, fill(gold)),
            ellipse(50, 20, fill(goldenrod)),
            ellipse(30, 20, fill(darkgoldenrod)),
            ellipse(10, 20, fill(sienna)),
            x_place=LEFT,
        )
    )
)
