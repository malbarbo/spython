from spython.image import pulled_regular_polygon, to_svg, stroke, red

print(to_svg(pulled_regular_polygon(50, 6, 0.5, 45.0, stroke(red))))
