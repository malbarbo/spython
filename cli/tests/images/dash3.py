from spython.image import line, to_svg, stroke, black

print(to_svg(line(100, 0, stroke(black, dash_array=[6, 3, 2, 3], width=2))))
