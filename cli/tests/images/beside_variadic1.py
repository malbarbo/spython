from spython.image import beside, ellipse, to_svg, TOP, fill, navy, blue, dodgerblue

print(
    to_svg(
        beside(
            ellipse(16, 60, fill(navy)),
            ellipse(16, 40, fill(blue)),
            ellipse(16, 20, fill(dodgerblue)),
            y_place=TOP,
        )
    )
)
