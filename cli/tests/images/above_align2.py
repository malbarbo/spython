from spython.image import (
    above_align,
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
        above_align(
            above_align(
                above_align(
                    ellipse(70, 20, fill(gold)), LEFT, ellipse(50, 20, fill(goldenrod))
                ),
                LEFT,
                ellipse(30, 20, fill(darkgoldenrod)),
            ),
            LEFT,
            ellipse(10, 20, fill(sienna)),
        )
    )
)
