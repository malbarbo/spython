from spython.image import bitmap_data_uri

try:
    bitmap_data_uri("data:image/png;base64,\ud800")
except ValueError as e:
    print(e)
