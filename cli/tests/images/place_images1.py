from spython.image import (
    circle,
    place_images,
    rectangle,
    to_svg,
    fill,
    goldenrod,
    white,
)

print(
    to_svg(
        place_images(
            rectangle(24, 24, fill(goldenrod)),
            [(18, 20), (0, 6), (14, 2)],
            [circle(4, fill(white)), circle(4, fill(white)), circle(4, fill(white))],
        )
    )
)
