from spython.image import star_polygon, to_svg, stroke, darkred

print(to_svg(star_polygon(40, 7, 3, stroke(darkred))))
