from spython import chocolate, circle, crop, fill, to_svg

print(to_svg(crop(circle(40, fill(chocolate)), 0, 0, 40, 40)))
