from spython.image import rectangle, to_svg, stroke, black

print(to_svg(rectangle(80, 40, stroke(black, dash_array=[2.5, 1.5]))))
