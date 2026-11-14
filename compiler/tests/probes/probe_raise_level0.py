def f(x: int) -> int:
    if x < 0:
        raise ValueError("negativo")
    return x
print(f(1))
