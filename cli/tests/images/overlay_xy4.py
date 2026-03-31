from spython.image import ellipse, overlay_xy, to_svg, fill, stroke, black, forestgreen

print(
    to_svg(
        overlay_xy(
            overlay_xy(
                ellipse(40, 40, stroke(black)),
                10,
                15,
                ellipse(10, 10, fill(forestgreen)),
            ),
            20,
            15,
            ellipse(10, 10, fill(forestgreen)),
        )
    )
)
