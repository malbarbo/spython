from spython.image import (
    place_image,
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
        place_image(
            rectangle(64, 64, fill(palegoldenrod)),
            0,
            0,
            rotate(triangle(48, fill(yellowgreen)), 180),
            x_place=LEFT,
            y_place=TOP,
        )
    )
)
