from spython.image import (
    place_image_align,
    rectangle,
    to_svg,
    triangle,
    RIGHT,
    BOTTOM,
    fill,
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
