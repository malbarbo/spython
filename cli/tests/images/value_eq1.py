from spython.color import Color, red
from spython.font import Font
from spython.style import fill, join, stroke

print(Color(255, 0, 0) == red)
print(Color(1, 2, 3) != Color(1, 2, 3))
print(Color(1, 2, 3) == Color(1, 2, 4))
print(Font(size=20) == Font(size=20))
print(Font(size=20) == Font(size=21))
print(join(fill(red), stroke(red)) == join(fill(red), stroke(red)))
print(fill(red) == stroke(red))
print(red == 3)
