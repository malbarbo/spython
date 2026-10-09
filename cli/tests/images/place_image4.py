from spython.image import Image, circle, empty_scene, place_image, to_svg, fill, red

scene: Image = empty_scene(84, 12)
for i in range(20):
    scene = place_image(scene, 4 + i * 4, 6, circle(2, fill(red)))
print(to_svg(scene))
