from spython.image import add_solid_curve, rectangle, to_svg, fill, stroke, black, red

print(
    to_svg(
        add_solid_curve(
            rectangle(100, 100, stroke(black)),
            20,
            20,
            0,
            0.333,
            80,
            80,
            0,
            0.333,
            fill(red),
        )
    )
)
