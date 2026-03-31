from spython.image import (
    circle,
    overlay_offset,
    rectangle,
    to_svg,
    fill,
    black,
    darkorange,
)

print(
    to_svg(
        overlay_offset(
            overlay_offset(
                rectangle(60, 20, fill(black)), -50, 0, circle(20, fill(darkorange))
            ),
            70,
            0,
            circle(20, fill(darkorange)),
        )
    )
)
