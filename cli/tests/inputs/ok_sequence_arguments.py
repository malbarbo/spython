from spython.image import Point, polygon, path, move_to, line_to, width, fill, red

points: list[Point] = [Point(0, 0), Point(10, 0), Point(0, 10)]
print(width(polygon(points, fill(red))))

pairs: list[tuple[int, int]] = [(0, 0), (20, 0), (0, 20)]
print(width(polygon(pairs, fill(red))))

commands = [move_to(0, 0), line_to(30, 0), line_to(0, 30)]
print(width(path(commands, True, fill(red))))
