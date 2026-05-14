from spython.image import Font, FontStyle, text, to_svg, fill, black
from spython.font import BOLD

print(
    to_svg(
        text(
            "Bold Italic Underline",
            fill(black),
            font=Font(
                size=24.0,
                style=FontStyle.ITALIC,
                weight=BOLD,
                underline=True,
            ),
        )
    )
)
