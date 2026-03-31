from spython.image import (
    polygon,
    rectangle,
    to_svg,
    underlay,
    fill,
    stroke,
    mediumseagreen,
    darkslategray,
)

print(
    to_svg(
        underlay(
            rectangle(80, 80, fill(mediumseagreen)),
            polygon(
                [(0, 0), (50, 0), (0, 50), (50, 50)],
                stroke(darkslategray, width=10, linecap="square", linejoin="miter"),
            ),
        )
    )
)
