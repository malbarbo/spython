from spython.image import (
    to_svg,
    flip_horizontal,
    line,
    rotate,
    scale,
    stroke,
    black,
)

l = line(6, 0, stroke(black))
for img in [
    rotate(l, 45),
    rotate(l, -45),
    rotate(l, 30),
    rotate(line(0, 0, stroke(black)), 45),
    scale(l, 4),
    scale(l, 0.5),
    flip_horizontal(scale(line(6, 4, stroke(black)), 2)),
]:
    print(to_svg(img))
