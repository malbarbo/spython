from spython.image import (
    circle,
    overlay,
    rectangle,
    to_svg,
    fill,
    black,
    darkorange,
)

print(
    to_svg(
        overlay(
            overlay(
                rectangle(60, 20, fill(black)),
                circle(20, fill(darkorange)),
                x_offset=-50,
                y_offset=0,
            ),
            circle(20, fill(darkorange)),
            x_offset=70,
            y_offset=0,
        )
    )
)
