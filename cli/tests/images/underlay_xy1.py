from spython.image import rectangle, to_svg, underlay_xy, stroke, black

print(
    to_svg(
        underlay_xy(
            rectangle(20, 20, stroke(black)), 20, 0, rectangle(20, 20, stroke(black))
        )
    )
)
