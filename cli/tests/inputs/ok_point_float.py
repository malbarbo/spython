from spython.image import Point, polygon, path, move_to, line_to, width, height, fill, red

print(Point(1, 2).x, Point(1, 2).y)

triangle = polygon([Point(0, 0), Point(10, 0), Point(0, 10)], fill(red))
print(width(triangle), height(triangle))

shape = path([move_to(0, 0), line_to(30, 0), line_to(0, 30)], True, fill(red))
print(width(shape), height(shape))
