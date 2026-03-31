from spython.image import place_line, rectangle, to_svg, fill, stroke, lightgray, maroon

print(
    to_svg(
        place_line(rectangle(40, 40, fill(lightgray)), -10, 50, 50, -10, stroke(maroon))
    )
)
