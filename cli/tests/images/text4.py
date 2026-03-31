from spython.image import Font, text, to_svg, fill, green

print(to_svg(text("Underlined", fill(green), font=Font(size=18.0, underline=True))))
