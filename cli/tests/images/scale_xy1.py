from spython.image import beside, ellipse, scale_xy, to_svg, fill, blue

print(
    to_svg(
        beside(scale_xy(ellipse(20, 30, fill(blue)), 3, 2), ellipse(60, 60, fill(blue)))
    )
)
