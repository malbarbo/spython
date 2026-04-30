from spython.image import circle, overlay, to_svg, fill, red, blue

print(
    to_svg(
        overlay(circle(40, fill(red)), circle(40, fill(blue)), x_offset=10, y_offset=10)
    )
)
