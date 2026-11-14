def divide(a: int, b: int) -> tuple[int, int]:
    return a // b, a % b
q, r = divide(7, 2)
print(q, r, divide(9, 4))
