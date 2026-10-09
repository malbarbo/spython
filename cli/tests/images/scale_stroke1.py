from spython.image import beside, rectangle, scale, scale_xy, stroke, to_svg, black

thick = rectangle(20, 20, stroke(black, width=2))
thin = rectangle(20, 20, stroke(black))
print(to_svg(beside(thick, scale(thick, 3), scale_xy(thick, 4, 1), scale(thin, 4))))
