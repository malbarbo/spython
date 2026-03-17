from spython import ellipse, fill, lightgray, put_image, rectangle, red, to_svg

print(
    to_svg(
        put_image(
            rectangle(50, 50, fill(lightgray)), 40, 15, ellipse(20, 30, fill(red))
        )
    )
)
