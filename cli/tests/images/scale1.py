from spython.image import beside, ellipse, scale, to_svg, fill, blue

print(
    to_svg(beside(scale(ellipse(20, 30, fill(blue)), 2), ellipse(40, 60, fill(blue))))
)
