from spython.image import (
    above_align,
    ellipse,
    to_svg,
    RIGHT,
    fill,
    yellowgreen,
    olivedrab,
    darkolivegreen,
    darkgreen,
)

print(
    to_svg(
        above_align(
            above_align(
                above_align(
                    ellipse(70, 20, fill(yellowgreen)),
                    RIGHT,
                    ellipse(50, 20, fill(olivedrab)),
                ),
                RIGHT,
                ellipse(30, 20, fill(darkolivegreen)),
            ),
            RIGHT,
            ellipse(10, 20, fill(darkgreen)),
        )
    )
)
