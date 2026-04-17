from dataclasses import dataclass


@dataclass
class Point:
    x: int
    y: int


@dataclass
class Rectangle:
    width: int
    height: int


def bounding_box(points: list[Point]) -> Rectangle:
    '''
    Returns the minimum bounding rectangle covering all *points*.
    If there are 1 or fewer points, returns a rectangle with width and height 0.

    Examples
    >>> bounding_box([])
    Rectangle(width=0, height=0)
    >>> bounding_box([Point(-3, -1)])
    Rectangle(width=0, height=0)
    >>> bounding_box([Point(-3, -1), Point(1, 1)])
    Rectangle(width=4, height=2)
    >>> bounding_box([Point(-3, -1), Point(1, 1), Point(2, -3)])
    Rectangle(width=5, height=4)
    >>> bounding_box([Point(-3, -1), Point(1, 1), Point(-10, 5), Point(2, -3), Point(4, 3), Point(9, 8)])
    Rectangle(width=19, height=11)
    '''
    if len(points) <= 1:
        r = Rectangle(0, 0)
    else:
        min_x = points[0].x
        max_x = points[0].x
        min_y = points[0].y
        max_y = points[0].y
        for p in points:
            if p.x < min_x:
                min_x = p.x
            elif p.x > max_x:
                max_x = p.x
            if p.y < min_y:
                min_y = p.y
            elif p.y > max_y:
                max_y = p.y
        r = Rectangle(max_x - min_x, max_y - min_y)
    return r


assert bounding_box([]) == Rectangle(0, 0)
assert bounding_box([Point(-3, -1)]) == Rectangle(0, 0)
assert bounding_box([Point(-3, -1), Point(1, 1)]) == Rectangle(4, 2)
assert bounding_box([Point(-3, -1), Point(1, 1), Point(2, -3)]) == Rectangle(5, 4)
assert bounding_box([Point(-3, -1), Point(1, 1), Point(-10, 5), Point(2, -3), Point(4, 3), Point(9, 8)]) == Rectangle(19, 11)
