from spython.image import (
    place_line,
    rectangle,
    to_svg,
    fill,
    stroke,
    darkolivegreen,
    goldenrod,
)

print(
    to_svg(
        place_line(
            rectangle(100, 100, fill(darkolivegreen)),
            25,
            25,
            100,
            100,
            stroke(goldenrod, width=30, linejoin="round", linecap="round"),
        )
    )
)
