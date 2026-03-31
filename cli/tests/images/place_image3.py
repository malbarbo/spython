from spython.image import circle, place_image, rectangle, to_svg, fill, goldenrod, white

print(
    to_svg(
        place_image(
            place_image(
                place_image(
                    place_image(
                        rectangle(24, 24, fill(goldenrod)),
                        8,
                        14,
                        circle(4, fill(white)),
                    ),
                    14,
                    2,
                    circle(4, fill(white)),
                ),
                0,
                6,
                circle(4, fill(white)),
            ),
            18,
            20,
            circle(4, fill(white)),
        )
    )
)
