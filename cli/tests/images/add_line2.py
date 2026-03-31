from spython.image import add_line, rectangle, to_svg, fill, stroke, gray, maroon

print(to_svg(add_line(rectangle(40, 40, fill(gray)), -10, 50, 50, -10, stroke(maroon))))
