from spython.image import ellipse, to_svg, underlay_xy, fill, lightgray, forestgreen

print(
    to_svg(
        underlay_xy(
            underlay_xy(
                ellipse(40, 40, fill(lightgray)),
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
