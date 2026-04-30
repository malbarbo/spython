from spython.image import (
    LEFT,
    RIGHT,
    TOP,
    BOTTOM,
    rhombus,
    star_polygon,
    to_svg,
    underlay,
    fill,
    navy,
    cornflowerblue,
)

star = star_polygon(20, 11, 3, fill(cornflowerblue))
print(
    to_svg(
        underlay(
            underlay(
                underlay(
                    underlay(
                        rhombus(120, 90, fill(navy)),
                        star,
                        x_offset=16,
                        y_offset=16,
                        x_place=LEFT,
                        y_place=TOP,
                    ),
                    star,
                    x_offset=-16,
                    y_offset=16,
                    x_place=RIGHT,
                    y_place=TOP,
                ),
                star,
                x_offset=16,
                y_offset=-16,
                x_place=LEFT,
                y_place=BOTTOM,
            ),
            star,
            x_offset=-16,
            y_offset=-16,
            x_place=RIGHT,
            y_place=BOTTOM,
        )
    )
)
