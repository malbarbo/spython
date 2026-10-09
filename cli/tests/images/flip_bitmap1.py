from spython.image import (
    beside,
    bitmap_data_uri,
    flip_horizontal,
    flip_vertical,
    to_svg,
)

pixels = bitmap_data_uri(
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGP4z8DA8J+BkYHh////DAAe9gT9Ce00PgAAAABJRU5ErkJggg=="
)
print(to_svg(beside(pixels, flip_horizontal(pixels), flip_vertical(pixels))))
