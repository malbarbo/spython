from spython.image import LEFT, MIDDLE, square, to_svg, underlay, fill, seagreen

s = fill(seagreen, opacity=0.25)
print(
    to_svg(
        underlay(
            square(50, s),
            square(40, s),
            square(30, s),
            square(20, s),
            x_place=LEFT,
            y_place=MIDDLE,
        )
    )
)
