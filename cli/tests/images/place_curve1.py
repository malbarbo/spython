from spython.image import empty_scene, place_curve, to_svg, stroke, blue

print(
    to_svg(
        place_curve(
            empty_scene(100, 100), 10, 50, 90, 0.5, 90, 50, 90, 0.5, stroke(blue)
        )
    )
)
