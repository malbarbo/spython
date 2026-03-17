from spython import fill, lightgray, place_image, rectangle, red, to_svg, triangle

print(
    to_svg(
        place_image(rectangle(48, 48, fill(lightgray)), 24, 24, triangle(32, fill(red)))
    )
)
