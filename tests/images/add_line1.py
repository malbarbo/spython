from spython import add_line, ellipse, maroon, stroke, to_svg

print(to_svg(add_line(ellipse(40, 40, stroke(maroon)), 0, 40, 40, 0, stroke(maroon))))
