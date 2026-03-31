from spython.image import rectangle, rotate, to_svg, stroke, black

print(to_svg(rotate(rectangle(50, 50, stroke(black)), 5)))
