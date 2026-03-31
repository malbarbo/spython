from spython.image import circle, to_svg, underlay_offset, fill, red, blue

print(to_svg(underlay_offset(circle(40, fill(red)), 10, 10, circle(40, fill(blue)))))
