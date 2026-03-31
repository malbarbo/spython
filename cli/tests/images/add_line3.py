from spython.image import (
    add_line,
    rectangle,
    to_svg,
    fill,
    stroke,
    darkolivegreen,
    goldenrod,
)

print(
    to_svg(
        add_line(
            rectangle(100, 100, fill(darkolivegreen)),
            25,
            25,
            75,
            75,
            stroke(goldenrod, width=30, linejoin="round", linecap="round"),
        )
    )
)
