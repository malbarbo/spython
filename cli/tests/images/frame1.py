from spython.image import ellipse, frame, to_svg, fill, gray

print(to_svg(frame(ellipse(40, 40, fill(gray)))))
