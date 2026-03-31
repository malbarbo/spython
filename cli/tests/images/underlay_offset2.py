from spython.image import circle, to_svg, underlay_offset, fill, gray, navy

print(
    to_svg(
        underlay_offset(
            circle(40, fill(gray)),
            0,
            -10,
            underlay_offset(circle(10, fill(navy)), -30, 0, circle(10, fill(navy))),
        )
    )
)
