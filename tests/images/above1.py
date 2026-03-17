from spython import above, black, darkgray, dimgray, ellipse, fill, lightgray, to_svg

print(
    to_svg(
        above(
            ellipse(70, 20, fill(lightgray)),
            ellipse(50, 20, fill(darkgray)),
            ellipse(30, 20, fill(dimgray)),
            ellipse(10, 20, fill(black)),
        )
    )
)
