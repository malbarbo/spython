from spython import beside, black, blue, fill, rectangle, scale, to_svg

print(
    to_svg(
        scale(
            beside(rectangle(100, 200, fill(black)), rectangle(200, 100, fill(blue))), 2
        )
    )
)
