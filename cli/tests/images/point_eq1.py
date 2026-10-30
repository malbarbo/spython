from spython.image import Point, center, circle, fill, red

print(center(circle(5, fill(red))) == Point(5, 5))
print(Point(1, 2) == Point(1, 2.5))
print(Point(1, 2.5))
