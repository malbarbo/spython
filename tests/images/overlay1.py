from spython import ellipse, fill, orange, overlay, purple, rectangle, to_svg

print(to_svg(overlay(rectangle(30, 60, fill(orange)), ellipse(60, 30, fill(purple)))))
