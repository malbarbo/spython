from spython import beside, blue, ellipse, fill, scale, to_svg

print(
    to_svg(beside(scale(ellipse(20, 30, fill(blue)), 2), ellipse(40, 60, fill(blue))))
)
