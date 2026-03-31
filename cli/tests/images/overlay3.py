from spython.image import overlay, regular_polygon, to_svg, fill, rgb

print(
    to_svg(
        overlay(
            overlay(
                overlay(
                    overlay(
                        regular_polygon(20, 5, fill(rgb(50, 50, 255))),
                        regular_polygon(26, 5, fill(rgb(100, 100, 255))),
                    ),
                    regular_polygon(32, 5, fill(rgb(150, 150, 255))),
                ),
                regular_polygon(38, 5, fill(rgb(200, 200, 255))),
            ),
            regular_polygon(44, 5, fill(rgb(250, 250, 255))),
        )
    )
)
