from spython import add_polygon, fill, forestgreen, lightblue, square, to_svg

print(
    to_svg(
        add_polygon(
            square(65, fill(lightblue)),
            [(30, -20), (50, 50), (-20, 30)],
            fill(forestgreen),
        )
    )
)
