from spython import circle, crop_align, to_svg, fill, LEFT, TOP, chocolate

print(to_svg(crop_align(circle(40, fill(chocolate)), LEFT, TOP, 40, 40)))
