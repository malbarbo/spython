from spython.image import add_wedge, circle, to_svg, fill, stroke, black, red

print(to_svg(add_wedge(circle(40, stroke(black)), 40, 40, 40, 90, fill(red))))
