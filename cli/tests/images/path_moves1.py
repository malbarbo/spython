from spython.image import (
    beside,
    dimension,
    line_to,
    move_to,
    path,
    square,
    stroke,
    red,
)

s = stroke(red)
# A move that no segment leaves draws nothing.
print(dimension(path([move_to(0, 0), line_to(10, 0), move_to(50, 50)], False, s)))
print(dimension(path([move_to(-40, 0), move_to(0, 0), line_to(10, 5)], False, s)))
print(dimension(path([move_to(0, 0), move_to(30, 30)], False, s)))
# A path that does not start with a move draws nothing.
print(dimension(path([line_to(10, 10), line_to(20, 0)], False, s)))
print(dimension(beside(square(10, s), path([move_to(0, 0)], False, s))))
