from spython.image import ellipse, put_image, rectangle, to_svg, fill, lightgray, red

print(
    to_svg(
        put_image(
            rectangle(50, 50, fill(lightgray)), 40, 15, ellipse(20, 30, fill(red))
        )
    )
)
