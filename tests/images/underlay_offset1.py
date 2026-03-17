from spython import blue, circle, fill, red, to_svg, underlay_offset

print(to_svg(underlay_offset(circle(40, fill(red)), 10, 10, circle(40, fill(blue)))))
