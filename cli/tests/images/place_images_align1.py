from spython.image import (
    place_images,
    rectangle,
    to_svg,
    triangle,
    RIGHT,
    BOTTOM,
    fill,
    goldenrod,
    yellowgreen,
)

print(
    to_svg(
        place_images(
            rectangle(64, 64, fill(goldenrod)),
            [(64, 64), (64, 48), (64, 32)],
            [
                triangle(48, fill(yellowgreen)),
                triangle(48, fill(yellowgreen)),
                triangle(48, fill(yellowgreen)),
            ],
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
