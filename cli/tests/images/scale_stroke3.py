from spython.image import beside, scale, scale_xy, stroke, text, to_svg, black

thick = text("Hi", stroke(black, width=2), size=20)
thin = text("Hi", stroke(black), size=20)
print(to_svg(beside(thick, scale(thick, 3), scale_xy(thick, 4, 1), scale(thin, 4))))
