from spython.image import add_polygon, square, to_svg, fill, lightblue, pink

print(
    to_svg(
        add_polygon(
            square(50, fill(lightblue)),
            [(25, -10), (60, 25), (25, 60), (-10, 25)],
            fill(pink),
        )
    )
)
