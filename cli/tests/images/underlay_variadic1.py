from spython.image import underlay, square, to_svg, RIGHT, BOTTOM, fill, gray

s = fill(gray, opacity=0.4)
print(
    to_svg(
        underlay(
            square(50, s),
            square(40, s),
            square(30, s),
            square(20, s),
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
