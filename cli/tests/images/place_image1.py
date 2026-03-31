from spython.image import place_image, rectangle, to_svg, triangle, fill, lightgray, red

print(
    to_svg(
        place_image(rectangle(48, 48, fill(lightgray)), 24, 24, triangle(32, fill(red)))
    )
)
