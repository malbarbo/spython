from spython.image import LEFT, MIDDLE, square, to_svg, underlay_align, fill, seagreen

s = fill(seagreen, opacity=0.25)
print(
    to_svg(
        underlay_align(
            underlay_align(
                underlay_align(square(50, s), LEFT, MIDDLE, square(40, s)),
                LEFT,
                MIDDLE,
                square(30, s),
            ),
            LEFT,
            MIDDLE,
            square(20, s),
        )
    )
)
