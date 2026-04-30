from spython.image import circle, to_svg, underlay, fill, red, blue

print(
    to_svg(
        underlay(
            circle(40, fill(red)), circle(40, fill(blue)), x_offset=10, y_offset=10
        )
    )
)
