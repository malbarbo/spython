from spython.image import rectangle, to_svg, underlay_xy, fill, red, black

print(
    to_svg(
        underlay_xy(
            rectangle(20, 20, fill(red)), -10, -10, rectangle(20, 20, fill(black))
        )
    )
)
