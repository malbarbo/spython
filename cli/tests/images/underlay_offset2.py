from spython.image import circle, to_svg, underlay, fill, gray, navy

print(
    to_svg(
        underlay(
            circle(40, fill(gray)),
            underlay(
                circle(10, fill(navy)),
                circle(10, fill(navy)),
                x_offset=-30,
                y_offset=0,
            ),
            x_offset=0,
            y_offset=-10,
        )
    )
)
