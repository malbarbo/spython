from spython.image import add_polygon, square, to_svg, fill, lightblue, forestgreen

print(
    to_svg(
        add_polygon(
            square(65, fill(lightblue)),
            [(30, -20), (50, 50), (-20, 30)],
            fill(forestgreen),
        )
    )
)
