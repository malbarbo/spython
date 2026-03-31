from spython.image import combine, ellipse, to_svg, underlay, fill, red

print(
    to_svg(
        combine(
            [
                ellipse(10, 60, fill(red, opacity=0.2)),
                ellipse(20, 50, fill(red, opacity=0.2)),
                ellipse(30, 40, fill(red, opacity=0.2)),
                ellipse(40, 30, fill(red, opacity=0.2)),
                ellipse(50, 20, fill(red, opacity=0.2)),
                ellipse(60, 10, fill(red, opacity=0.2)),
            ],
            underlay,
        )
    )
)
