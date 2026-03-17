from spython import ellipse, fill, frame, gray, to_svg

print(to_svg(frame(ellipse(40, 40, fill(gray)))))
