from spython import blue, circle, fill, overlay_offset, red, to_svg

print(to_svg(overlay_offset(circle(40, fill(red)), 10, 10, circle(40, fill(blue)))))
