from spython.image import beside, ellipse, rotate, scale_xy, to_svg, fill, blue

e = rotate(ellipse(40, 10, fill(blue)), 45)
print(to_svg(beside(e, scale_xy(e, 2, 1), scale_xy(e, 1, 3))))
