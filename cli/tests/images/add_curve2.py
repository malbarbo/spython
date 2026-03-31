from spython.image import add_curve, rectangle, to_svg, stroke, black, red

print(
    to_svg(
        add_curve(
            rectangle(100, 100, stroke(black)),
            50,
            10,
            270,
            0.5,
            50,
            90,
            90,
            0.5,
            stroke(red),
        )
    )
)
