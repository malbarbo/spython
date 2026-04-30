from spython.image import (
    above,
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
        above(
            ellipse(70, 20, fill(yellowgreen)),
            ellipse(50, 20, fill(olivedrab)),
            ellipse(30, 20, fill(darkolivegreen)),
            ellipse(10, 20, fill(darkgreen)),
            x_place=RIGHT,
        )
    )
)
