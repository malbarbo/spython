from spython.image import beside, rectangle, scale, scale_xy, stroke, to_svg, black

dashed = rectangle(20, 20, stroke(black, width=2, dash_array=[4, 2]))
print(to_svg(beside(dashed, scale(dashed, 3), scale_xy(dashed, 4, 1))))
