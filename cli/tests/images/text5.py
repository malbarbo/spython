from spython.image import Font, FontStyle, FontWeight, text, to_svg, fill, black

print(
    to_svg(
        text(
            "Bold Italic Underline",
            fill(black),
            font=Font(
                size=24.0,
                style=FontStyle.ITALIC,
                weight=FontWeight.BOLD,
                underline=True,
            ),
        )
    )
)
