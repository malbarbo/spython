from spython.image import ellipse, rotate, to_svg, fill, olivedrab

print(to_svg(rotate(ellipse(60, 20, fill(olivedrab)), 45)))
