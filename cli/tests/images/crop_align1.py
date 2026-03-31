from spython.image import circle, crop_align, to_svg, LEFT, TOP, fill, chocolate

print(to_svg(crop_align(circle(40, fill(chocolate)), LEFT, TOP, 40, 40)))
