from spython.image import circle, overlay, to_svg, fill, rgba

print(
    to_svg(
        overlay(
            overlay(
                circle(30, fill(rgba(0, 150, 0, 0.5))),
                circle(30, fill(rgba(0, 0, 255, 0.5))),
                x_offset=26,
                y_offset=0,
            ),
            circle(30, fill(rgba(200, 0, 0, 0.5))),
            x_offset=0,
            y_offset=26,
        )
    )
)
