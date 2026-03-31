from spython.image import ellipse, overlay, to_svg, fill, red, black

print(
    to_svg(
        overlay(
            overlay(
                overlay(
                    overlay(
                        overlay(
                            ellipse(10, 10, fill(red)), ellipse(20, 20, fill(black))
                        ),
                        ellipse(30, 30, fill(red)),
                    ),
                    ellipse(40, 40, fill(black)),
                ),
                ellipse(50, 50, fill(red)),
            ),
            ellipse(60, 60, fill(black)),
        )
    )
)
