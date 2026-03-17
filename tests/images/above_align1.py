from spython import (
    above_align,
    ellipse,
    to_svg,
    fill,
    RIGHT,
    yellowgreen,
    olivedrab,
    darkolivegreen,
    darkgreen,
)

img = ellipse(70, 20, fill(yellowgreen))
img = above_align(img, RIGHT, ellipse(50, 20, fill(olivedrab)))
img = above_align(img, RIGHT, ellipse(30, 20, fill(darkolivegreen)))
img = above_align(img, RIGHT, ellipse(10, 20, fill(darkgreen)))
print(to_svg(img))
