from spython.image import Font, text, to_svg, fill, red
from spython.font import BOLD

print(to_svg(text("Bold text", fill(red), font=Font(size=20.0, weight=BOLD))))
