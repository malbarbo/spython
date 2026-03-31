from spython.image import beside, rectangle, rotate, to_svg, fill, darkseagreen

print(
    to_svg(
        rotate(
            beside(
                rectangle(40, 20, fill(darkseagreen)),
                rectangle(20, 100, fill(darkseagreen)),
            ),
            45,
        )
    )
)
