from spython.image import overlay_xy, rectangle, to_svg, stroke, black

print(
    to_svg(
        overlay_xy(
            rectangle(20, 20, stroke(black)), 20, 0, rectangle(20, 20, stroke(black))
        )
    )
)
