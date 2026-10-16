from spython.image import (
    Point,
    add_polygon,
    beside,
    circle,
    combine,
    empty_scene,
    fill,
    line_to,
    move_to,
    path,
    place_images,
    place_polygon,
    polygon,
    red,
    width,
)

points: list[Point] = [Point(0, 0), Point(10, 0), Point(0, 10)]
print(width(polygon(points, fill(red))))

pairs: list[tuple[int, int]] = [(0, 0), (20, 0), (0, 20)]
print(width(polygon(pairs, fill(red))))

commands = [move_to(0, 0), line_to(30, 0), line_to(0, 30)]
print(width(path(commands, True, fill(red))))

print(width(place_polygon(empty_scene(40, 40), points, fill(red))))
print(width(add_polygon(empty_scene(40, 40), pairs, fill(red))))

circles = (circle(5, fill(red)), circle(5, fill(red)))
print(width(place_images(empty_scene(40, 40), points[:2], circles)))
print(width(combine(circles, beside)))
