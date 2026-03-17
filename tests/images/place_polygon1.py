from spython import fill, forestgreen, lightblue, place_polygon, square, to_svg

print(
    to_svg(
        place_polygon(
            square(65, fill(lightblue)),
            [(30, -20), (50, 50), (-20, 30)],
            fill(forestgreen),
        )
    )
)
