from spython import ellipse, fill, olivedrab, rotate, to_svg

print(to_svg(rotate(ellipse(60, 20, fill(olivedrab)), 45)))
