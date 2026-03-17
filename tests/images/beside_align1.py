from spython import (
    beside_align,
    ellipse,
    to_svg,
    fill,
    BOTTOM,
    lightsteelblue,
    mediumslateblue,
    slateblue,
    navy,
)

img = ellipse(20, 70, fill(lightsteelblue))
img = beside_align(img, BOTTOM, ellipse(20, 50, fill(mediumslateblue)))
img = beside_align(img, BOTTOM, ellipse(20, 30, fill(slateblue)))
img = beside_align(img, BOTTOM, ellipse(20, 10, fill(navy)))
print(to_svg(img))
