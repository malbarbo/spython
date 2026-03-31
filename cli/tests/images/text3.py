from spython.image import Font, FontStyle, text, to_svg, fill, blue

print(
    to_svg(
        text("Italic text", fill(blue), font=Font(size=20.0, style=FontStyle.ITALIC))
    )
)
