from spython import (
    ellipse,
    rectangle,
    to_svg,
    underlay_align,
    fill,
    LEFT,
    MIDDLE,
    orange,
    purple,
)

print(
    to_svg(
        underlay_align(
            rectangle(30, 60, fill(orange)), LEFT, MIDDLE, ellipse(60, 30, fill(purple))
        )
    )
)
