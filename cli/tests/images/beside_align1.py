from spython.image import (
    beside,
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
        beside(
            ellipse(20, 70, fill(lightsteelblue)),
            ellipse(20, 50, fill(mediumslateblue)),
            ellipse(20, 30, fill(slateblue)),
            ellipse(20, 10, fill(navy)),
            y_place=BOTTOM,
        )
    )
)
