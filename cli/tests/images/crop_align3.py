from spython.image import (
    circle,
    crop_align,
    to_svg,
    CENTER,
    MIDDLE,
    fill,
    mediumslateblue,
)

print(to_svg(crop_align(circle(25, fill(mediumslateblue)), CENTER, MIDDLE, 50, 30)))
