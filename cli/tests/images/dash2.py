from spython.image import rectangle, to_svg, stroke, blue

print(to_svg(rectangle(80, 40, stroke(blue, dash_array=[2, 2]))))
