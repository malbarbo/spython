from spython.image import Font, text, to_svg, fill, darkgray
from spython.font import LIGHT

print(
    to_svg(
        text(
            "Light mono",
            fill(darkgray),
            font=Font(family="monospace", size=16.0, weight=LIGHT),
        )
    )
)
