from spython.image import beside, fill, black, red, text, to_svg

print(
    to_svg(
        beside(
            text("", fill(black), size=16),
            text("a", fill(red), size=16),
            text("", fill(black), size=16),
        )
    )
)
