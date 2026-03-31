from spython.image import beside, rectangle, scale, to_svg, fill, black, blue

print(
    to_svg(
        scale(
            beside(rectangle(100, 200, fill(black)), rectangle(200, 100, fill(blue))), 2
        )
    )
)
