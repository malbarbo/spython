from spython.image import circle, to_svg, stroke, red

print(to_svg(circle(30, stroke(red, dash_array=[6, 3]))))
