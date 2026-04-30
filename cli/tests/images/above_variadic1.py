from spython.image import above, ellipse, to_svg, LEFT, fill, navy, blue, dodgerblue

print(
    to_svg(
        above(
            ellipse(60, 16, fill(navy)),
            ellipse(40, 16, fill(blue)),
            ellipse(20, 16, fill(dodgerblue)),
            x_place=LEFT,
        )
    )
)
