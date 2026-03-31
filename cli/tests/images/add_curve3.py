from spython.image import add_curve, rectangle, to_svg, stroke, black, red

print(
    to_svg(
        add_curve(
            rectangle(100, 100, stroke(black)),
            20,
            50,
            0,
            0.0,
            80,
            50,
            0,
            0.0,
            stroke(red),
        )
    )
)
