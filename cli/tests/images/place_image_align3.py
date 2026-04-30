from spython.image import (
    beside,
    circle,
    place_image,
    rectangle,
    to_svg,
    fill,
    stroke,
    black,
    tomato,
)

print(
    to_svg(
        beside(
            place_image(rectangle(32, 32, stroke(black)), 0, 0, circle(8, fill(tomato))),
            place_image(rectangle(32, 32, stroke(black)), 8, 8, circle(8, fill(tomato))),
            place_image(
                rectangle(32, 32, stroke(black)), 16, 16, circle(8, fill(tomato))
            ),
            place_image(
                rectangle(32, 32, stroke(black)), 24, 24, circle(8, fill(tomato))
            ),
            place_image(
                rectangle(32, 32, stroke(black)), 32, 32, circle(8, fill(tomato))
            ),
        )
    )
)
