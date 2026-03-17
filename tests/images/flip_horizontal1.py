from spython import beside, blue, fill, flip_horizontal, red, rotate, square, to_svg

print(
    to_svg(
        beside(
            rotate(square(50, fill(red)), 30),
            flip_horizontal(rotate(square(50, fill(blue)), 30)),
        )
    )
)
