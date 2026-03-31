from spython.image import text, to_svg, fill, black

print(to_svg(text("Testing text", fill(black), size=16)))
