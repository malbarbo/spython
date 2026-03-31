from spython.image import polygon, to_svg, fill, plum

print(
    to_svg(
        polygon(
            [
                (0, 0),
                (0, 40),
                (20, 40),
                (20, 60),
                (40, 60),
                (40, 20),
                (20, 20),
                (20, 0),
            ],
            fill(plum),
        )
    )
)
