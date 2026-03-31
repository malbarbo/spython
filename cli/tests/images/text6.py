from spython.image import overlay, rectangle, text, to_svg, fill, white, blue

print(
    to_svg(overlay(text("Hello", fill(white), size=20), rectangle(80, 30, fill(blue))))
)
