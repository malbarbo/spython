from spython import ellipse, maroon, place_line, stroke, to_svg

print(to_svg(place_line(ellipse(40, 40, stroke(maroon)), 0, 40, 40, 0, stroke(maroon))))
