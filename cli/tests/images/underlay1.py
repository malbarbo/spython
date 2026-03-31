from spython.image import ellipse, rectangle, to_svg, underlay, fill, orange, purple

print(to_svg(underlay(rectangle(30, 60, fill(orange)), ellipse(60, 30, fill(purple)))))
