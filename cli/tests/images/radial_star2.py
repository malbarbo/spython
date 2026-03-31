from spython.image import radial_star, to_svg, stroke, black

print(to_svg(radial_star(32, 30, 40, stroke(black))))
