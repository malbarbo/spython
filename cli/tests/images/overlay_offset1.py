from spython.image import circle, overlay_offset, to_svg, fill, red, blue

print(to_svg(overlay_offset(circle(40, fill(red)), 10, 10, circle(40, fill(blue)))))
