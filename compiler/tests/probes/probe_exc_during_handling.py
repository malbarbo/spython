def zero() -> int:
    return 0
try:
    1 // zero()
except ZeroDivisionError:
    xs: list[int] = []
    print(xs[0])
