from spython import black, overlay_xy, rectangle, stroke, to_svg

print(
    to_svg(
        overlay_xy(
            rectangle(20, 20, stroke(black)), 20, 0, rectangle(20, 20, stroke(black))
        )
    )
)
