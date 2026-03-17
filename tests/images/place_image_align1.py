from spython import (
    place_image_align,
    rectangle,
    to_svg,
    triangle,
    fill,
    RIGHT,
    BOTTOM,
    palegoldenrod,
    yellowgreen,
)

print(
    to_svg(
        place_image_align(
            rectangle(64, 64, fill(palegoldenrod)),
            64,
            64,
            RIGHT,
            BOTTOM,
            triangle(48, fill(yellowgreen)),
        )
    )
)
