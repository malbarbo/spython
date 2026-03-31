from spython.image import ellipse, place_line, to_svg, stroke, maroon

print(to_svg(place_line(ellipse(40, 40, stroke(maroon)), 0, 40, 40, 0, stroke(maroon))))
