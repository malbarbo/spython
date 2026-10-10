from spython.image import to_svg, beside, ellipse, stroke, black

print(to_svg(beside(ellipse(0, 20, stroke(black)), ellipse(20, 0, stroke(black)))))
