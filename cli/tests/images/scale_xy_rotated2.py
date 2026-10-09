from spython.image import (
    beside,
    crop,
    frame,
    rectangle,
    rotate,
    scale_xy,
    text,
    to_svg,
    fill,
    blue,
)

t = rotate(text("abc", size=20), 45)
c = rotate(crop(rectangle(40, 40, fill(blue)), 0, 0, 30, 10), 45)
print(
    to_svg(
        beside(frame(t), frame(scale_xy(t, 3, 1)), frame(c), frame(scale_xy(c, 3, 1)))
    )
)
