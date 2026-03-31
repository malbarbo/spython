from spython.image import polygon, to_svg, fill, burlywood

print(to_svg(polygon([(0, 0), (-10, 20), (60, 0), (-10, -20)], fill(burlywood))))
