from spython.image import (
    place_image,
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
        place_image(
            rectangle(64, 64, fill(palegoldenrod)),
            64,
            64,
            triangle(48, fill(yellowgreen)),
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
