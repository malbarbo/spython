from spython.image import add_curve, rectangle, to_svg, stroke, black, red

print(
    to_svg(
        add_curve(
            rectangle(100, 100, stroke(black)),
            20,
            20,
            0,
            0.333,
            80,
            80,
            0,
            0.333,
            stroke(red),
        )
    )
)
