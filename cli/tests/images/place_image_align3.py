from spython.image import (
    beside,
    circle,
    combine,
    place_image_align,
    rectangle,
    to_svg,
    CENTER,
    MIDDLE,
    fill,
    stroke,
    black,
    tomato,
)

print(
    to_svg(
        combine(
            [
                place_image_align(
                    rectangle(32, 32, stroke(black)),
                    0,
                    0,
                    CENTER,
                    MIDDLE,
                    circle(8, fill(tomato)),
                ),
                place_image_align(
                    rectangle(32, 32, stroke(black)),
                    8,
                    8,
                    CENTER,
                    MIDDLE,
                    circle(8, fill(tomato)),
                ),
                place_image_align(
                    rectangle(32, 32, stroke(black)),
                    16,
                    16,
                    CENTER,
                    MIDDLE,
                    circle(8, fill(tomato)),
                ),
                place_image_align(
                    rectangle(32, 32, stroke(black)),
                    24,
                    24,
                    CENTER,
                    MIDDLE,
                    circle(8, fill(tomato)),
                ),
                place_image_align(
                    rectangle(32, 32, stroke(black)),
                    32,
                    32,
                    CENTER,
                    MIDDLE,
                    circle(8, fill(tomato)),
                ),
            ],
            beside,
        )
    )
)
