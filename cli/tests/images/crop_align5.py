from spython.image import (
    beside,
    rectangle,
    crop_align,
    frame,
    to_svg,
    LEFT,
    CENTER,
    RIGHT,
    TOP,
    MIDDLE,
    BOTTOM,
    fill,
    red,
)

square = rectangle(10, 10, fill(red))
print(
    to_svg(
        beside(
            frame(crop_align(square, LEFT, TOP, 20, 20)),
            frame(crop_align(square, CENTER, MIDDLE, 20, 20)),
            frame(crop_align(square, RIGHT, BOTTOM, 20, 20)),
        )
    )
)
