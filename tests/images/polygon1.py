from spython import burlywood, fill, polygon, to_svg

print(to_svg(polygon([(0, 0), (-10, 20), (60, 0), (-10, -20)], fill(burlywood))))
