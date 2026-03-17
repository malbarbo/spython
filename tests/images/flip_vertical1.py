from spython import above, fill, firebrick, flip_vertical, gray, scale_xy, star, to_svg

print(
    to_svg(
        above(
            star(40, fill(firebrick)),
            scale_xy(flip_vertical(star(40, fill(gray))), 1.0, 0.5),
        )
    )
)
