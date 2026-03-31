from spython.image import (
    beside_align,
    ellipse,
    to_svg,
    BOTTOM,
    fill,
    lightsteelblue,
    mediumslateblue,
    slateblue,
    navy,
)

print(
    to_svg(
        beside_align(
            beside_align(
                beside_align(
                    ellipse(20, 70, fill(lightsteelblue)),
                    BOTTOM,
                    ellipse(20, 50, fill(mediumslateblue)),
                ),
                BOTTOM,
                ellipse(20, 30, fill(slateblue)),
            ),
            BOTTOM,
            ellipse(20, 10, fill(navy)),
        )
    )
)
