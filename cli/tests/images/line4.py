from spython.image import to_svg, beside, line, stroke, black, red

print(to_svg(beside(line(0, 20, stroke(black)), line(20, 0, stroke(red)))))
