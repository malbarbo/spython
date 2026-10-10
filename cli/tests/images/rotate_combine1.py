from spython.image import to_svg, beside, combine, rectangle, rotate, fill, red

print(to_svg(rotate(combine([rectangle(40, 20, fill(red))], beside), 90)))
