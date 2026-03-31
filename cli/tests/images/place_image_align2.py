from spython.image import (
    place_image_align,
    rectangle,
    rotate,
    to_svg,
    triangle,
    LEFT,
    TOP,
    fill,
    palegoldenrod,
    yellowgreen,
)

print(
    to_svg(
        place_image_align(
            rectangle(64, 64, fill(palegoldenrod)),
            0,
            0,
            LEFT,
            TOP,
            rotate(triangle(48, fill(yellowgreen)), 180),
        )
    )
)
