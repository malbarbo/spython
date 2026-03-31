from spython.image import combine, ellipse, to_svg, underlay, fill, red, black

print(
    to_svg(
        combine(
            [
                ellipse(10, 60, fill(red)),
                ellipse(20, 50, fill(black)),
                ellipse(30, 40, fill(red)),
                ellipse(40, 30, fill(black)),
                ellipse(50, 20, fill(red)),
                ellipse(60, 10, fill(black)),
            ],
            underlay,
        )
    )
)
