from spython.image import (
    beside,
    ellipse,
    to_svg,
    TOP,
    fill,
    mediumorchid,
    darkorchid,
    purple,
    indigo,
)

print(
    to_svg(
        beside(
            ellipse(20, 70, fill(mediumorchid)),
            ellipse(20, 50, fill(darkorchid)),
            ellipse(20, 30, fill(purple)),
            ellipse(20, 10, fill(indigo)),
            y_place=TOP,
        )
    )
)
