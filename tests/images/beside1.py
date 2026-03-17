from spython import beside, black, darkgray, dimgray, ellipse, fill, lightgray, to_svg

print(
    to_svg(
        beside(
            ellipse(20, 70, fill(lightgray)),
            ellipse(20, 50, fill(darkgray)),
            ellipse(20, 30, fill(dimgray)),
            ellipse(20, 10, fill(black)),
        )
    )
)
