class Point:
    x: int
    y: int

    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

    def scale(self, factor: int) -> "Point":
        return Point(self.x * factor, self.y * factor)


p = Point(3, 4).scale(2)
print(p.x, p.y)
