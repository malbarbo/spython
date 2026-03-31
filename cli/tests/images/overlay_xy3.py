from spython.image import overlay_xy, rectangle, to_svg, fill, red, black

print(
    to_svg(
        overlay_xy(
            rectangle(20, 20, fill(red)), -10, -10, rectangle(20, 20, fill(black))
        )
    )
)
