from spython import black, rectangle, stroke, to_svg, underlay_xy

print(
    to_svg(
        underlay_xy(
            rectangle(20, 20, stroke(black)), 20, 0, rectangle(20, 20, stroke(black))
        )
    )
)
