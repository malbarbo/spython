from spython.image import beside, blue, fill, rectangle, red, rotate, square

a = square(10, fill(red))
print(a == square(10, fill(red)))
print(a != square(10, fill(red)))
print(a == square(11, fill(red)))
print(a == beside(rectangle(5, 10, fill(red)), rectangle(5, 10, fill(red))))
print(a == rotate(a, 90))
print(a == 3)
try:
    print(rectangle(1e9, 1e9, fill(red)) == rectangle(1e9, 1e9, fill(blue)))
except ValueError as e:
    print(e)
