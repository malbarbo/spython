from spython.image import add_line, ellipse, to_svg, stroke, maroon

print(to_svg(add_line(ellipse(40, 40, stroke(maroon)), 0, 40, 40, 0, stroke(maroon))))
