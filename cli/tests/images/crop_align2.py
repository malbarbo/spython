from spython.image import crop_align, ellipse, to_svg, RIGHT, BOTTOM, fill, dodgerblue

print(to_svg(crop_align(ellipse(80, 120, fill(dodgerblue)), RIGHT, BOTTOM, 40, 60)))
