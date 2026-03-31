from spython.image import place_polygon, square, to_svg, fill, stroke, yellow, darkblue

print(
    to_svg(
        place_polygon(
            square(180, fill(yellow)),
            [(109, 160), (26, 148), (46, 36), (93, 44), (89, 68), (122, 72)],
            stroke(darkblue),
        )
    )
)
