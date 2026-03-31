from spython.image import crop, ellipse, to_svg, fill, dodgerblue

print(to_svg(crop(ellipse(80, 120, fill(dodgerblue)), 40, 60, 40, 60)))
