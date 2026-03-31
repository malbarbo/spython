from spython.image import (
    ellipse,
    overlay_align,
    rectangle,
    to_svg,
    LEFT,
    MIDDLE,
    fill,
    orange,
    purple,
)

print(
    to_svg(
        overlay_align(
            rectangle(30, 60, fill(orange)), LEFT, MIDDLE, ellipse(60, 30, fill(purple))
        )
    )
)
