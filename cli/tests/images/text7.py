from spython.image import Font, FontWeight, text, to_svg, fill, darkgray

print(
    to_svg(
        text(
            "Light mono",
            fill(darkgray),
            font=Font(family="monospace", size=16.0, weight=FontWeight.LIGHT),
        )
    )
)
