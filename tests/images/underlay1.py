from spython import ellipse, fill, orange, purple, rectangle, to_svg, underlay

print(to_svg(underlay(rectangle(30, 60, fill(orange)), ellipse(60, 30, fill(purple)))))
