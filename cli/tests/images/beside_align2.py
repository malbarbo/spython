from spython.image import (
    beside_align,
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
        beside_align(
            beside_align(
                beside_align(
                    ellipse(20, 70, fill(mediumorchid)),
                    TOP,
                    ellipse(20, 50, fill(darkorchid)),
                ),
                TOP,
                ellipse(20, 30, fill(purple)),
            ),
            TOP,
            ellipse(20, 10, fill(indigo)),
        )
    )
)
