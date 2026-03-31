from spython.image import (
    above,
    flip_vertical,
    scale_xy,
    star,
    to_svg,
    fill,
    firebrick,
    gray,
)

print(
    to_svg(
        above(
            star(40, fill(firebrick)),
            scale_xy(flip_vertical(star(40, fill(gray))), 1.0, 0.5),
        )
    )
)
