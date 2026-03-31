from spython.image import Font, FontWeight, text, to_svg, fill, red

print(
    to_svg(text("Bold text", fill(red), font=Font(size=20.0, weight=FontWeight.BOLD)))
)
