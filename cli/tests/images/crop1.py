from spython.image import circle, crop, to_svg, fill, chocolate

print(to_svg(crop(circle(40, fill(chocolate)), 0, 0, 40, 40)))
