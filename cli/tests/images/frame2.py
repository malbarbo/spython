from spython.image import (
    beside,
    ellipse,
    frame,
    to_svg,
    fill,
    lightsteelblue,
    mediumslateblue,
    slateblue,
    navy,
)

print(
    to_svg(
        beside(
            beside(
                beside(
                    ellipse(20, 70, fill(lightsteelblue)),
                    frame(ellipse(20, 50, fill(mediumslateblue))),
                ),
                ellipse(20, 30, fill(slateblue)),
            ),
            ellipse(20, 10, fill(navy)),
        )
    )
)
