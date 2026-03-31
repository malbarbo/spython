from spython.image import (
    place_images_align,
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
        place_images_align(
            rectangle(64, 64, fill(goldenrod)),
            [(64, 64), (64, 48), (64, 32)],
            RIGHT,
            BOTTOM,
            [
                triangle(48, fill(yellowgreen)),
                triangle(48, fill(yellowgreen)),
                triangle(48, fill(yellowgreen)),
            ],
        )
    )
)
