def f(x: int, xs: list[int] = []) -> list[int]:
    xs.append(x)
    return xs
print(f(1), f(2))
